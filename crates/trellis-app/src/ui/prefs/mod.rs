//! Full-window Preferences: a navigation column, an optional
//! sub-list (Integrations), and a form with right-aligned labels and help text.

mod appearance;
mod editor;
mod general;
mod git;
mod git_program;
mod gitflow;
mod integrations;
mod keyboard;
mod keyboard_row;
mod lfs;
mod links;
mod profile_setup;
mod profiles;
mod signing;
mod sparse;
mod ssh;
mod ssh_generate;
mod ssh_git;
mod ssh_service;
mod theme_row;
mod tools;
mod trello;
mod updates_row;

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::{AppState, PrefPage};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Ui, pos2, vec2};
use trellis_core::forge::Provider;

pub(super) const LABEL_W: f32 = 240.0;

/// One form row: right-aligned label, then the control(s) and help text underneath.
pub(super) fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, 24.0), Sense::hover());
        ui.painter().text(pos2(r.right(), r.center().y), Align2::RIGHT_CENTER, label, FontId::proportional(13.5), color::text());
        ui.add_space(18.0);
        ui.vertical(|ui| {
            ui.set_max_width(480.0);
            add(ui);
        });
    });
    ui.add_space(18.0);
}

pub(super) fn help(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(color::weak())).wrap());
}

pub(super) fn warn(ui: &mut Ui, text: &str) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
        let c = r.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![pos2(c.x, c.y - 5.0), pos2(c.x + 5.5, c.y + 4.5), pos2(c.x - 5.5, c.y + 4.5)],
            Color32::TRANSPARENT,
            Stroke::new(1.3_f32, color::amber()),
        ));
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(color::weak())).wrap());
    });
}

pub(super) fn caption(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(text).size(12.5).color(color::weak()));
    });
    ui.add_space(4.0);
}

pub(super) fn text_field(ui: &mut Ui, value: &mut String, hint: &str) -> egui::Response {
    ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(320.0).margin(vec2(8.0, 5.0)))
}

pub(super) fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    let fill = if enabled { color::primary() } else { color::primary_off() };
    let fg = if enabled { Color32::WHITE } else { color::faint() };
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(fg)).fill(fill).stroke(Stroke::new(1.0_f32, color::primary_border())),
    )
}

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(page) = st.prefs.as_ref().map(|p| p.page) else { return };
    let repo_name = st.snap.as_ref().map(|s| s.name.clone());
    nav(ctx, page, repo_name.as_deref(), st, msgs);
    if page == PrefPage::Integrations {
        providers(ctx, st, msgs);
    }

    let lfs_cx = lfs::LfsContext {
        repo_name: repo_name.clone(),
        remote: st.snap.as_ref().and_then(crate::state::lfs::lfs_remote),
        job: st.job.as_ref().map(|j| j.label.clone()),
    };
    let sparse_cx = sparse::SparseContext { repo_name: repo_name.clone(), job: st.job.as_ref().map(|j| j.label.clone()) };
    let gitflow_cx = gitflow::GitFlowContext { snap: st.snap.as_ref(), busy: st.job.is_some() };
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
            let title = if page == PrefPage::Integrations { prefs.int_provider.name() } else { page.title() };
            ui.label(RichText::new(title).size(21.0).color(color::text()));
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
                        ui.label(RichText::new("Reading git configuration…").color(color::weak()));
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
                    PrefPage::Ui => appearance::ui_page(ui, settings, themes, now, msgs),
                    PrefPage::Editor => editor::editor_page(ui, settings),
                    PrefPage::Keyboard => keyboard::keyboard_page(ui, &mut prefs.keys, keymap, msgs),
                    PrefPage::Tools => tools::tools_page(ui, settings, msgs),
                    PrefPage::RepoLfs => lfs::lfs_page(ui, &mut prefs.lfs, &lfs_cx, msgs),
                    PrefPage::RepoGitFlow => gitflow::gitflow_page(ui, &mut prefs.gitflow, &gitflow_cx, msgs),
                    PrefPage::RepoSparse => sparse::sparse_page(ui, &mut prefs.sparse, &sparse_cx, msgs),
                    PrefPage::Ssh => ssh::ssh_page(ui, &mut prefs.ssh, &ssh_cx, msgs),
                    _ => {
                        let Some(git) = prefs.git.as_mut() else { return };
                        match page {
                            PrefPage::Profile => profiles::profile_page(ui, &*settings, &mut prefs.profiles, git, switching, msgs),
                            PrefPage::RepoIdentity => git::identity_page(ui, git, msgs),
                            _ => git::hooks_page(ui, git, msgs),
                        }
                    }
                }
            });
        });
}

fn nav(ctx: &egui::Context, page: PrefPage, repo_name: Option<&str>, st: &mut AppState, msgs: &mut Vec<Msg>) {
    egui::SidePanel::left("prefs-nav").exact_width(250.0).resizable(false).frame(egui::Frame::new().fill(color::panel())).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(6.0);
        if widgets::nav_row(ui, Some(Icon::Back), "Exit Preferences", false, 15.0).on_hover_text("Esc").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::ClosePrefs));
        }
        ui.add_space(12.0);
        caption(ui, "Preferences");
        let mut go = |ui: &mut Ui, pg: PrefPage, icon: Icon, label: &str| {
            if widgets::nav_row(ui, Some(icon), label, page == pg, 15.0).clicked()
                && let Some(p) = st.prefs.as_mut()
            {
                // A key being recorded or a warning belongs to the page it was on.
                if p.page != pg {
                    p.keys.recording = None;
                    p.keys.pending = None;
                    p.keys.confirm_reset_all = false;
                }
                p.page = pg;
            }
        };
        go(ui, PrefPage::General, Icon::Gear, "General");
        // Not `go`: the page looks for installed gits when it opens.
        if widgets::nav_row(ui, Some(Icon::Branch), "Git", page == PrefPage::Git, 15.0).clicked() {
            msgs.push(Msg::GitEnv(crate::git_env::GitEnvMsg::ShowPage));
        }
        // Not `go`: the page seeds and fills its profiles when it opens.
        if widgets::nav_row(ui, Some(Icon::User), "Profile", page == PrefPage::Profile, 15.0).clicked() {
            msgs.push(Msg::Profile(crate::profile::ProfileMsg::ShowPage));
        }
        go(ui, PrefPage::Integrations, Icon::Plug, "Integrations");
        go(ui, PrefPage::Ui, Icon::Brush, "UI Customization");
        go(ui, PrefPage::Editor, Icon::Code, "Editor");
        go(ui, PrefPage::Keyboard, Icon::Keyboard, "Keyboard");
        go(ui, PrefPage::Tools, Icon::Tools, "External Tools");
        go(ui, PrefPage::Signing, Icon::Key, "Commit Signing");
        // Not `go`: the page reads ~/.ssh when it opens.
        if widgets::nav_row(ui, Some(Icon::Lock), "SSH", page == PrefPage::Ssh, 15.0).clicked() {
            msgs.push(Msg::Ssh(crate::ssh::SshMsg::ShowPage));
        }
        if let Some(name) = repo_name {
            ui.add_space(20.0);
            caption(ui, "Repo-Specific Preferences");
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(format!("Repo: {name}")).size(12.0).color(color::faint()));
            });
            ui.add_space(4.0);
            go(ui, PrefPage::RepoIdentity, Icon::Identity, "Identity");
            go(ui, PrefPage::RepoHooks, Icon::Hook, "Git Hooks");
            // Not `go`: the page loads Trello boards when it opens.
            if widgets::nav_row(ui, Some(Icon::Activity), "Issue Tracker", page == PrefPage::RepoTracker, 15.0).clicked() {
                msgs.push(Msg::Forge(crate::msg::ForgeMsg::ShowTrackerPage));
            }
            // Not `go`: the page reads its status when it opens.
            if widgets::nav_row(ui, Some(Icon::Storage), "Git LFS", page == PrefPage::RepoLfs, 15.0).clicked() {
                msgs.push(Msg::Lfs(crate::lfs::LfsMsg::ShowPage));
            }
            // Not `go`: the page fills its form from the snapshot.
            if widgets::nav_row(ui, Some(Icon::Flow), "Git Flow", page == PrefPage::RepoGitFlow, 15.0).clicked() {
                msgs.push(Msg::GitFlow(crate::gitflow::GitFlowMsg::ShowPage));
            }
            // Not `go`: the page reads its status when it opens.
            if widgets::nav_row(ui, Some(Icon::Sparse), "Sparse Checkout", page == PrefPage::RepoSparse, 15.0).clicked() {
                msgs.push(Msg::Sparse(crate::msg::SparseMsg::ShowPage));
            }
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
                if widgets::nav_row(ui, None, &label, current == Some(p), 14.0).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::SelectIntegration(p)));
                }
            }
        },
    );
}
