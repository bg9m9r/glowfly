//! Full-window Preferences: a navigation column, an optional sub-list (Integrations), and a
//! form with right-aligned labels and help text (form.rs), the page title with its "Saved" tick,
//! and the Save / Revert bar under a page with unsaved edits (save_bar.rs).

mod appearance;
mod appearance_graph;
mod diff_viewer;
mod form;
mod general;
mod general_git;
mod git;
mod git_program;
mod gitflow;
mod integrations;
mod integrations_form;
mod keyboard;
mod keyboard_row;
mod language;
mod lfs;
mod links;
mod profile_setup;
mod profiles;
mod save_bar;
mod signing;
mod sparse;
mod ssh;
mod ssh_agent;
mod ssh_card;
mod ssh_git;
mod ssh_service;
pub(super) mod terminal;
mod theme_row;
mod tools;
mod trello;
mod updates_row;

pub(super) use form::*;

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::prefs_save;
use crate::state::{AppState, PrefPage};
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use trellis_core::forge::Provider;
use trellis_core::i18n::Lang;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(page) = st.prefs.as_ref().map(|p| p.page) else { return };
    let t = st.settings.lang();
    let repo_name = st.snap.as_ref().map(|s| s.name.clone());
    nav(ctx, page, repo_name.as_deref(), t, msgs);
    if page == PrefPage::Integrations {
        providers(ctx, st, msgs);
    }
    if st.prefs.as_ref().is_some_and(|p| prefs_save::unsaved(p, &st.settings, st.snap.as_ref())) {
        // Only Git Flow's and sparse checkout's saves run as a git job.
        let busy = st.job.is_some() && matches!(page, PrefPage::RepoGitFlow | PrefPage::RepoSparse);
        save_bar::show(ctx, t, save_bar::BarView { busy }, msgs);
    }

    let lfs_cx = lfs::LfsContext {
        repo_name: repo_name.clone(),
        remote: st.snap.as_ref().and_then(crate::state::lfs::lfs_remote),
        job: st.job.as_ref().map(|j| j.label.clone()),
    };
    let save_first = t.tr("prefs.save_bar.first");
    let sparse_cx = sparse::SparseContext { repo_name: repo_name.clone(), job: st.job.as_ref().map(|j| j.label.clone()), save_first };
    let gitflow_cx = gitflow::GitFlowContext { snap: st.snap.as_ref(), busy: st.job.is_some(), save_first };
    let ssh_cx = ssh::SshContext {
        use_agent: st.settings.use_ssh_agent,
        services: crate::state::ssh_service::services(&st.settings.connections)
            .into_iter()
            .map(|c| (c.account.clone(), crate::state::ssh_service::service_label(&c.account)))
            .collect(),
        job: st.job.as_ref().map(|j| j.label.clone()),
    };
    let updates = updates_row::UpdatesView {
        updates: &st.updates,
        last_checked: st.settings.updates.last_checked.map(|t| st.settings.format_time(t, st.view.now, st.view.utc_offset)),
    };
    let switching = st.profile_switch;
    let menu_bar_row = !st.view.chrome.drawn();
    let git_state = &st.git;
    let settings = &mut st.settings;
    let keymap = &st.keymap;
    let scheme = &st.deep_link.scheme;
    let Some(prefs) = st.prefs.as_mut() else { return };
    let fs = st.forge.as_ref();
    // Repository settings are keyed by where it really is (`settings::repo_keys`).
    let workdir = st.snap.as_ref().map(|s| s.real_workdir.clone());
    let now = (st.view.now, st.view.utc_offset);
    let themes = &st.themes;
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
            ui.add_space(18.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
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
                    PrefPage::Ui => appearance::ui_page(ui, settings, themes, menu_bar_row, msgs),
                    PrefPage::Language => language::language_page(ui, settings, now),
                    PrefPage::DiffViewer => diff_viewer::diff_viewer_page(ui, settings),
                    PrefPage::Keyboard => keyboard::keyboard_page(ui, t, &mut prefs.keys, keymap, msgs),
                    PrefPage::Tools => tools::tools_page(ui, settings, msgs),
                    PrefPage::Terminal => terminal::terminal_page(ui, settings, msgs),
                    PrefPage::RepoLfs => lfs::lfs_page(ui, &mut prefs.lfs, &lfs_cx, msgs),
                    PrefPage::RepoGitFlow => gitflow::gitflow_page(ui, &mut prefs.gitflow, &gitflow_cx, msgs),
                    PrefPage::RepoSparse => sparse::sparse_page(ui, &mut prefs.sparse, &sparse_cx, msgs),
                    PrefPage::Ssh => ssh::ssh_page(ui, t, &mut prefs.ssh, &ssh_cx, msgs),
                    _ => {
                        let Some(git) = prefs.git.as_mut() else { return };
                        match page {
                            PrefPage::Profile => profiles::profile_page(ui, &*settings, &mut prefs.profiles, git, switching, msgs),
                            PrefPage::RepoIdentity => git::identity_page(ui, git),
                            _ => git::hooks_page(ui, git, msgs),
                        }
                    }
                }
            });
        });
}

fn nav(ctx: &egui::Context, page: PrefPage, repo_name: Option<&str>, t: Lang, msgs: &mut Vec<Msg>) {
    egui::SidePanel::left("prefs-nav").exact_width(250.0).resizable(false).frame(egui::Frame::new().fill(color::panel())).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(6.0);
        let back = if repo_name.is_some() { t.tr("prefs.nav.back_to_repo") } else { t.tr("prefs.nav.back") };
        let r = widgets::nav_row(ui, Some(Icon::Back), back, false);
        // Its text is painted; name the row for screen readers.
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, back));
        if r.on_hover_text("Esc").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::ClosePrefs));
        }
        ui.add_space(12.0);
        caption(ui, t.tr("prefs.nav.caption"));
        // Every row goes through update, which asks first when the page has unsaved edits.
        let mut go = |ui: &mut Ui, pg: PrefPage, icon: Icon| {
            if widgets::nav_row(ui, Some(icon), pg.nav_label(t), page == pg).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::Navigate(pg)));
            }
        };
        go(ui, PrefPage::General, Icon::Gear);
        go(ui, PrefPage::Git, Icon::Branch);
        go(ui, PrefPage::Profile, Icon::User);
        go(ui, PrefPage::Integrations, Icon::Plug);
        go(ui, PrefPage::Ui, Icon::Brush);
        go(ui, PrefPage::Language, Icon::Globe);
        go(ui, PrefPage::DiffViewer, Icon::Code);
        go(ui, PrefPage::Keyboard, Icon::Keyboard);
        go(ui, PrefPage::Tools, Icon::Tools);
        go(ui, PrefPage::Terminal, Icon::Terminal);
        go(ui, PrefPage::Signing, Icon::Key);
        go(ui, PrefPage::Ssh, Icon::Lock);
        if let Some(name) = repo_name {
            ui.add_space(20.0);
            caption(ui, t.tr("prefs.nav.repo_caption"));
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(t.trf("prefs.nav.repo_name", &[("name", &name)])).size(type_scale::SMALL).color(color::faint()));
            });
            ui.add_space(4.0);
            go(ui, PrefPage::RepoIdentity, Icon::Identity);
            go(ui, PrefPage::RepoHooks, Icon::Hook);
            go(ui, PrefPage::RepoTracker, Icon::Activity);
            go(ui, PrefPage::RepoLfs, Icon::Storage);
            go(ui, PrefPage::RepoGitFlow, Icon::Flow);
            go(ui, PrefPage::RepoSparse, Icon::Sparse);
        }
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
