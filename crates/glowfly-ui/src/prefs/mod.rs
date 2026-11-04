//! Full-window Preferences: a navigation column with the search (nav.rs, index.rs), an optional
//! sub-list (Integrations), and a form at most 760 px wide with right-aligned labels and help
//! text (form.rs, form_help.rs), the page title with its "Saved" tick, and the Save / Revert bar
//! under a page with unsaved edits (save_bar.rs).

mod accessibility;
mod appearance;
mod appearance_graph;
mod diff_viewer;
mod form;
mod form_help;
mod general;
mod general_git;
mod general_notify;
mod git;
mod git_program;
mod gitflow;
mod group_colour_row;
mod index;
mod integrations;
mod integrations_form;
mod keyboard;
mod keyboard_row;
mod language;
mod lfs;
mod links;
mod nav;
mod profile_rules;
mod profile_setup;
mod profiles;
mod reveal;
mod save_bar;
mod signing;
mod sparse;
pub(super) mod ssh;
pub(super) mod terminal;
mod terminal_colours;
mod theme_files;
mod theme_picker;
mod theme_row;
mod tools;
mod trello;
mod updates_row;
// For the chip catalog golden (`snapshot_tests_chips.rs`).
#[cfg(test)]
pub(crate) use ssh::agent::pill_widget as ssh_agent_pill;
#[cfg(test)]
pub(crate) use ssh::card::chip as ssh_card_chip;

pub(super) use form::*;

use super::theme::color;
use super::widgets::{self, scroll_cue};
use crate::theme::type_scale;
use eframe::egui::{self, RichText};
use glowfly_core::forge::Provider;
use glowfly_model::msg::Msg;
use glowfly_model::msg::PrefsMsg;
use glowfly_model::state::prefs_save;
use glowfly_model::state::{AppState, PrefPage};

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(page) = st.prefs.as_ref().map(|p| p.page) else { return };
    let t = st.settings.lang();
    let repo_name = st.snap.as_ref().map(|s| s.name.clone());
    // Before the nav, which may set a new target this frame.
    reveal::sync(ctx, page, st.prefs.as_ref().is_some_and(|p| p.leaving.is_some()));
    if let Some(p) = st.prefs.as_mut() {
        let view = nav::NavView { page, repo_name: repo_name.as_deref(), t };
        nav::show(ctx, &view, &mut p.search, &mut p.focus_search, msgs);
    }
    form_help::set_lang(ctx, t);
    // Opened help texts close when the page changes.
    let shown_key = egui::Id::new("prefs-help-page");
    if ctx.data(|d| d.get_temp::<PrefPage>(shown_key)) != Some(page) {
        form_help::close_all(ctx);
        ctx.data_mut(|d| d.insert_temp(shown_key, page));
    }
    if page == PrefPage::Integrations {
        providers(ctx, st, msgs);
    }
    st.view.prefs_bar_focused = false;
    if st.prefs.as_ref().is_some_and(|p| prefs_save::unsaved(p, &st.settings, st.snap.as_ref())) {
        // Only Git Flow's and sparse checkout's saves run as a git job.
        let busy = st.job.is_some() && matches!(page, PrefPage::RepoGitFlow | PrefPage::RepoSparse);
        let saving = st.prefs.as_ref().is_some_and(prefs_save::saving);
        st.view.prefs_bar_focused = save_bar::show(ctx, t, save_bar::BarView { busy, saving }, msgs);
    }

    // While git is held in the repository, its pages read nothing with git (trust).
    let held = st.snap.as_ref().filter(|_| !st.runs_git_unasked()).map(|s| s.real_workdir.clone());
    let lfs_cx = lfs::LfsContext {
        repo_name: repo_name.clone(),
        remote: st.snap.as_ref().and_then(glowfly_model::state::lfs::lfs_remote),
        job: st.job.as_ref().map(|j| j.label.clone()),
        held: held.clone(),
        patterns: st.snap.as_ref().map(|s| s.lfs_patterns.clone()).unwrap_or_default(),
        lang: t,
    };
    let save_first = t.tr("prefs.save_bar.first");
    let job = st.job.as_ref().map(|j| j.label.clone());
    let tools_held = held.is_some();
    let sparse_cx = sparse::SparseContext { repo_name: repo_name.clone(), job, save_first, held };
    let gitflow_cx = gitflow::GitFlowContext { snap: st.snap.as_ref(), busy: st.job.is_some(), save_first };
    let ssh_cx = ssh::SshContext {
        use_agent: st.settings.use_ssh_agent,
        services: glowfly_model::state::ssh::service::services(&st.settings.connections)
            .into_iter()
            .map(|c| (c.account.clone(), glowfly_model::state::ssh::service::service_label(&c.account)))
            .collect(),
        job: st.job.as_ref().map(|j| j.label.clone()),
        extra: st.settings.ssh_extra_keys.clone(),
    };
    let updates = updates_row::UpdatesView {
        updates: &st.updates,
        last_checked: st.settings.updates.last_checked.map(|t| st.settings.format_time(t, st.cues.now, st.cues.utc_offset)),
    };
    let switching = st.profile_switch;
    // Field by field, beside the `&mut` borrows of the settings and Preferences below.
    let here = st.snap.as_ref().map(|s| profile_rules::RepoHere {
        name: &s.name,
        git_dir: &s.git_dir,
        common: &s.common.dir,
        identity: st.identities.get(&s.git_dir),
        healed: st.includes.healed.contains(&s.git_dir),
    });
    let frame_rows = appearance::FrameRows { menu_bar: !st.cues.chrome.drawn(), system_title_bar: st.cues.system_title_bar_offered };
    let on_network = st.snap.as_ref().is_some_and(|s| st.watch.remote.contains_key(&s.workdir));
    let monitor = git::MonitorRow {
        offered: st.git.monitor_offer_for(glowfly_core::opens::Os::current()) == glowfly_core::repo::MonitorOffer::Daemon && !on_network,
        busy: st.job.is_some(),
    };
    let git_state = &st.git;
    let settings = &mut st.settings;
    let keymap = &st.keymap;
    let scheme = &st.deep_link.scheme;
    let Some(prefs) = st.prefs.as_mut() else { return };
    let fs = st.session.forge.as_ref();
    // Repository settings are keyed by where it really is (`settings::repo_keys`).
    let workdir = st.snap.as_ref().map(|s| s.real_workdir.clone());
    let now = (st.cues.now, st.cues.utc_offset);
    let themes = &st.themes;
    let a11y = accessibility::A11yView {
        keys: keymap,
        sheet_open: matches!(st.modal, Some(glowfly_model::state::Modal::Shortcuts { .. })),
        text_size_followable: st.cues.system_a11y.text_size_followable,
    };
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(egui::Margin { left: 24, right: 24, top: 18, bottom: 18 }))
        .show(ctx, |ui| {
            let title = if page == PrefPage::Integrations { prefs.int_provider.name() } else { page.title(t) };
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).font(type_scale::strong(ui, type_scale::TITLE)).color(color::text()));
                if prefs.saved.showing() {
                    ui.add_space(12.0);
                    save_bar::saved_tick(ui, t, prefs.saved.alpha);
                }
            });
            // A save git refused: its error, until the page is saved or reverted.
            if let Some(e) = prefs_save::save_error(prefs) {
                ui.add_space(6.0);
                warn(ui, &t.trf("prefs.save.error", &[("error", &e)]));
            }
            ui.add_space(18.0);
            scroll_cue::show(ui, egui::ScrollArea::vertical().auto_shrink([false, false]), color::bg(), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                // Left-anchored and at most CONTENT_W wide, however wide the window.
                ui.set_max_width(CONTENT_W.min(ui.available_width()));
                let needs_git = matches!(page, PrefPage::Profile | PrefPage::Signing | PrefPage::RepoIdentity | PrefPage::RepoHooks);
                if page == PrefPage::Signing && prefs.git.is_some() {
                    signing::signing_page(ui, settings, prefs, msgs);
                    return;
                }
                if needs_git && prefs.git.is_none() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(RichText::new(t.tr("prefs.common.reading_git_config")).color(color::weak()));
                    });
                    return;
                }
                match page {
                    PrefPage::General => {
                        let cx = general::GeneralContext { program: git_state.info.as_ref(), keys: keymap, scheme, updates };
                        general::general_page(ui, settings, prefs.git.as_mut(), &cx, msgs)
                    }
                    PrefPage::Git => git_program::git_program_page(ui, settings, git_state, msgs),
                    PrefPage::Integrations => integrations::integration_page(ui, settings, prefs, fs, msgs),
                    PrefPage::RepoTracker => integrations::tracker_page(ui, settings, prefs, fs, workdir.as_deref(), msgs),
                    PrefPage::Ui => appearance::ui_page(ui, settings, themes, keymap, frame_rows, msgs),
                    PrefPage::Accessibility => accessibility::accessibility_page(ui, settings, &a11y, msgs),
                    PrefPage::Language => language::language_page(ui, settings, now),
                    PrefPage::DiffViewer => diff_viewer::diff_viewer_page(ui, settings),
                    PrefPage::Keyboard => keyboard::keyboard_page(ui, t, &mut prefs.keys, keymap, msgs),
                    PrefPage::Tools => {
                        let git = tools::GitToolsView { tools: prefs.git_tools.as_ref(), held: tools_held };
                        tools::tools_page(ui, settings, &git, msgs)
                    }
                    PrefPage::Terminal => terminal::terminal_page(ui, settings, themes, msgs),
                    PrefPage::RepoLfs => lfs::lfs_page(ui, &mut prefs.lfs, &lfs_cx, msgs),
                    PrefPage::RepoGitFlow => gitflow::gitflow_page(ui, &mut prefs.gitflow, &gitflow_cx, msgs),
                    PrefPage::RepoSparse => sparse::sparse_page(ui, &mut prefs.sparse, &sparse_cx, msgs),
                    PrefPage::Ssh => ssh::ssh_page(ui, t, &mut prefs.ssh, &ssh_cx, msgs),
                    _ => {
                        let Some(git) = prefs.git.as_mut() else { return };
                        match page {
                            PrefPage::Profile => profiles::profile_page(ui, &*settings, &mut prefs.profiles, git, switching, here, msgs),
                            PrefPage::RepoIdentity => git::identity_page(ui, git, here.and_then(|h| h.identity), settings),
                            _ => git::hooks_page(ui, git, &monitor, t, msgs),
                        }
                    }
                }
            });
        });
}

/// The Integrations sub-list of hosting services.
fn providers(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let current = st.prefs.as_ref().map(|p| p.int_provider);
    let connected: Vec<Provider> = st.settings.connections.iter().map(|c| c.account.provider).collect();
    egui::SidePanel::left("prefs-integrations").exact_width(230.0).resizable(false).frame(egui::Frame::new().fill(color::toolbar())).show(
        ctx,
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(8.0);
            for p in Provider::ALL {
                let label = if connected.contains(&p) { format!("{}  ✔", p.name()) } else { p.name().to_owned() };
                if widgets::nav_row(ui, None, &label, current == Some(p)).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::SelectIntegration(p)));
                }
            }
        },
    );
}
