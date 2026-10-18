//! Top toolbar: repository and branch pickers, git actions, and app buttons.

use super::common::{caret_after, menu_item, short};
use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::{BindingId, Keymap};
use crate::msg::BranchMsg;
use crate::msg::CloneMsg;
use crate::msg::ForgeMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::PublishMsg;
use crate::msg::RecentMsg;
use crate::msg::RepoMsg;
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use eframe::egui::{self, Align, Layout, RichText};
use trellis_core::forge::Provider;
use trellis_core::i18n::Lang;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let labels = st.settings.toolbar_labels;
    let tool = |ui: &mut egui::Ui, icon, label: &str, name: &str, enabled| widgets::tool_button(ui, icon, label, name, enabled, labels);
    egui::TopBottomPanel::top("toolbar")
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 4)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(48.0);
                // repository ▾ › branch ▾
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(t.tr("toolbar.repository")).size(11.0).color(color::weak()));
                    let name = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.tr("toolbar.open").into());
                    let r = ui.menu_button(RichText::new(name).size(16.0).strong().color(color::text()), |ui| repo_menu(ui, t, st, msgs));
                    caret_after(ui, &r.response);
                });
                let Some(snap) = &st.snap else { return };
                ui.add_space(12.0);
                ui.label(RichText::new("›").size(20.0).color(color::weak()));
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(t.tr("toolbar.branch")).size(11.0).color(color::weak()));
                    let head = snap
                        .head_branch
                        .clone()
                        .unwrap_or_else(|| snap.head_oid.map(|o| t.trf("toolbar.detached", &[("sha", &short(o))])).unwrap_or_default());
                    let r = ui.menu_button(RichText::new(head).size(16.0).color(color::text()), |ui| {
                        ui.set_min_width(220.0);
                        for b in &snap.locals {
                            let text = if b.is_head { format!("✔ {}", b.name) } else { b.name.clone() };
                            let elsewhere = crate::state::worktree_branches::checkout_blocked(snap, &b.name);
                            let mut r = ui.add_enabled(!b.is_head && elsewhere.is_none(), egui::Button::new(text));
                            if let Some(why) = &elsewhere {
                                r = r.on_disabled_hover_text(why);
                            }
                            if r.clicked() {
                                msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.clone())));
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        let new_branch = Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() });
                        menu_item(msgs, ui, t.tr("toolbar.new_branch"), new_branch);
                    });
                    caret_after(ui, &r.response);
                });
                super::conflict_check::indicator(ui, st, msgs);

                ui.add_space(52.0);
                let idle = st.job.is_none();
                let undo_tip = st.undo.undo_tooltip(t, &st.keymap);
                if tool(ui, Icon::Undo, t.tr("toolbar.undo.button"), t.tr("toolbar.undo.name"), idle && !st.undo.undo.is_empty())
                    .on_hover_text(undo_tip)
                    .clicked()
                {
                    msgs.push(Msg::Job(JobMsg::Undo));
                }
                let redo_tip = st.undo.redo_tooltip(t, &st.keymap);
                if tool(ui, Icon::Redo, t.tr("toolbar.redo.button"), t.tr("toolbar.redo.name"), idle && !st.undo.redo.is_empty())
                    .on_hover_text(redo_tip)
                    .clicked()
                {
                    msgs.push(Msg::Job(JobMsg::Redo));
                }
                ui.add_space(6.0);
                if tool(ui, Icon::Pull, t.tr("toolbar.pull.button"), t.tr("toolbar.pull.name"), idle)
                    .on_hover_text(t.tr("toolbar.pull.tip"))
                    .clicked()
                {
                    msgs.push(Msg::git("Pull", &["pull", "--ff"]));
                }
                dropdown(ui, "pull-menu", t.tr("toolbar.pull_menu"), 240.0, |ui| {
                    // The button's text, and the job's label (English: it is also log and undo text).
                    for (text, label, a) in [
                        (t.tr("toolbar.fetch_all"), "Fetch all", &["fetch", "--all", "--prune"][..]),
                        (t.tr("toolbar.pull_ff"), "Pull (fast-forward if possible)", &["pull", "--ff"][..]),
                        (t.tr("toolbar.pull_ff_only"), "Pull (fast-forward only)", &["pull", "--ff-only"][..]),
                        (t.tr("toolbar.pull_rebase"), "Pull (rebase)", &["pull", "--rebase"][..]),
                    ] {
                        if ui.add_enabled(idle, egui::Button::new(text)).clicked() {
                            msgs.push(Msg::git(label, a));
                        }
                    }
                });
                if tool(ui, Icon::Push, t.tr("toolbar.push.button"), t.tr("toolbar.push.name"), idle).clicked() {
                    msgs.push(Msg::Job(JobMsg::Push { force: false }));
                }
                let host = st.forge.as_ref().filter(|f| f.host.is_some());
                dropdown(ui, "push-menu", t.tr("toolbar.push_menu"), if host.is_some() { 260.0 } else { 200.0 }, |ui| {
                    if ui.add_enabled(idle, egui::Button::new(t.tr("toolbar.force_push"))).clicked() {
                        msgs.push(Msg::confirm(
                            t.tr("toolbar.force_push.title"),
                            t.tr("toolbar.force_push.body").into(),
                            t.tr("toolbar.force_push.button"),
                            Msg::Job(JobMsg::Push { force: true }),
                        ));
                    }
                    if snap.remotes.is_empty() && ui.add_enabled(idle, egui::Button::new(t.tr("toolbar.publish"))).clicked() {
                        msgs.push(Msg::Publish(PublishMsg::PublishDialog));
                    }
                    if let Some(fs) = host {
                        ui.separator();
                        super::pr_menu_items::push_pr_item(ui, snap, fs, idle, msgs);
                    }
                });
                if tool(ui, Icon::Branch, t.tr("toolbar.branch.button"), t.tr("toolbar.branch.name"), idle).clicked() {
                    msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                }
                if tool(ui, Icon::Stash, t.tr("toolbar.stash.button"), t.tr("toolbar.stash.name"), idle && snap.is_dirty()).clicked() {
                    msgs.push(Msg::git("Stash", &["stash", "push", "-u"]));
                }
                if tool(ui, Icon::Pop, t.tr("toolbar.pop.button"), t.tr("toolbar.pop.name"), idle && !snap.stashes.is_empty()).clicked() {
                    msgs.extend(crate::state::stash_confirm::pop_latest(snap));
                }
                if let Some(fs) = host {
                    let can_pr = idle && snap.head_branch.is_some();
                    let service = fs.host_provider().map(Provider::name).unwrap_or_default();
                    let merge_request = fs.pr_noun() == "merge request";
                    let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
                    let (tip, name) = if merge_request {
                        (t.trf("toolbar.pr.tip_merge_request", args), t.trf("toolbar.pr.name_merge_request", args))
                    } else {
                        (t.trf("toolbar.pr.tip_pull_request", args), t.trf("toolbar.pr.name_pull_request", args))
                    };
                    // The name starts with the caption, so speech input finds the button by what it shows.
                    if tool(ui, Icon::PullRequest, t.tr("toolbar.pr.button"), &name, can_pr).on_hover_text(&tip).clicked()
                        && let Some(b) = &snap.head_branch
                    {
                        msgs.push(Msg::Forge(ForgeMsg::CreatePrDialog { source: b.clone(), target: None }));
                    }
                    let name = if merge_request { t.tr("toolbar.pr_menu.merge_request") } else { t.tr("toolbar.pr_menu.pull_request") };
                    dropdown(ui, "pr-menu", name, 260.0, |ui| super::pr_menu_items::pr_menu(ui, snap, fs, idle, msgs));
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                let tip = terminal_tip(t, &st.keymap, st.terminal.open, st.settings.terminal.open_in_panel);
                terminal_button(ui, t, st.terminal.open, labels, &tip, super::terminal::right_click_item(t, &st.settings), msgs);
                if tool(ui, Icon::Folder, t.tr("toolbar.folder.button"), t.tr("toolbar.folder.name"), true).clicked() {
                    msgs.push(Msg::OpenFolder(snap.workdir.clone()));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let prefs_tip = st.keymap.tip(t.tr("toolbar.settings.tip"), BindingId::Prefs);
                    if tool(ui, Icon::Gear, t.tr("toolbar.settings.button"), t.tr("toolbar.settings.name"), true)
                        .on_hover_text(prefs_tip)
                        .clicked()
                    {
                        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
                    }
                    let palette = t.tr("toolbar.commands.tip");
                    let pal = tool(ui, Icon::Palette, t.tr("toolbar.commands.button"), t.tr("toolbar.commands.name"), true);
                    if pal.on_hover_text(st.keymap.tip(palette, BindingId::Palette)).clicked() {
                        msgs.push(Msg::Palette(PaletteMsg::TogglePalette));
                    }
                    let search_tip = st.keymap.tip(t.tr("toolbar.search.tip"), BindingId::Search);
                    if tool(ui, Icon::Search, t.tr("toolbar.search.button"), t.tr("toolbar.search.name"), true)
                        .on_hover_text(search_tip)
                        .clicked()
                    {
                        st.view.show_search = !st.view.show_search;
                    }
                    if tool(ui, Icon::Activity, t.tr("toolbar.activity.button"), t.tr("toolbar.activity.name"), true).clicked() {
                        st.view.show_log = !st.view.show_log;
                    }
                    let refresh_tip = st.keymap.tip(t.tr("toolbar.refresh.tip"), BindingId::Refresh);
                    if tool(ui, Icon::Refresh, t.tr("toolbar.refresh.button"), t.tr("toolbar.refresh.name"), true)
                        .on_hover_text(refresh_tip)
                        .clicked()
                    {
                        msgs.push(Msg::Repo(RepoMsg::Refresh));
                    }
                    ui.add_space(6.0);
                    super::trust_chip::show(ui, st, msgs);
                });
            });
        });
}

/// The terminal button's tooltip, with the current key of Show or hide the terminal.
pub(super) fn terminal_tip(t: Lang, keys: &Keymap, open: bool, in_panel: bool) -> String {
    let key = keys.paren(BindingId::ToggleTerminal);
    let key = [("key", &key as &dyn std::fmt::Display)];
    match (open, in_panel) {
        (true, false) => t.trf("toolbar.terminal.hide_tip", &key),
        (false, false) => t.trf("toolbar.terminal.show_tip", &key),
        (true, true) => t.trf("toolbar.terminal.hide_tab_tip", &key),
        (false, true) => t.trf("toolbar.terminal.show_tab_tip", &key),
    }
}

/// Shows or hides the terminal panel; right-click offers `item` (Open in terminal: an external
/// one, or a new tab in the panel). A bar under the button marks the panel as open.
fn terminal_button(ui: &mut egui::Ui, t: Lang, open: bool, labels: bool, tip: &str, item: &str, msgs: &mut Vec<Msg>) {
    let name = if open { t.tr("toolbar.terminal.name_hide") } else { t.tr("toolbar.terminal.name_show") };
    let resp = widgets::tool_button(ui, Icon::Terminal, t.tr("toolbar.terminal.button"), name, true, labels).on_hover_text(tip);
    if open {
        let r = resp.rect;
        let bar = egui::Rect::from_min_max(egui::pos2(r.left() + 6.0, r.bottom() - 2.0), egui::pos2(r.right() - 6.0, r.bottom()));
        ui.painter().rect_filled(bar, 1.0, color::accent());
    }
    if resp.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::Toggle));
    }
    resp.context_menu(|ui| menu_item(msgs, ui, item, Msg::OpenTerminal));
}

/// The small ▾ button next to a tool button, opening a menu below it. The painted caret gets
/// `name` as its accessible name (screen readers and UI tests).
fn dropdown(ui: &mut egui::Ui, id: &str, name: &str, width: f32, contents: impl FnOnce(&mut egui::Ui)) {
    let caret = widgets::caret_button(ui);
    caret.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let popup = ui.make_persistent_id(id);
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &caret, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(width);
        contents(ui);
    });
}

fn repo_menu(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.set_min_width(260.0);
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.open"), Msg::Repo(RepoMsg::PickRepo));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.clone"), Msg::Clone(CloneMsg::Dialog));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.init"), Msg::Repo(RepoMsg::InitRepo));
    ui.separator();
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.dashboard"), Msg::Dashboard(crate::msg::DashboardMsg::Open));
    super::repo_menu::actions(ui, t, st, msgs);
    if let Some(snap) = &st.snap {
        let fav = crate::state::repo_lists::is_favorite(st, &snap.workdir);
        let label = if fav { t.tr("toolbar.repo_menu.favorite_remove") } else { t.tr("toolbar.repo_menu.favorite_add") };
        menu_item(msgs, ui, label, Msg::Recent(RecentMsg::SetFavorite { path: snap.workdir.clone(), favorite: !fav }));
    }
    super::repo_list::menu_sections(ui, st, msgs);
}
