//! Top toolbar: repository and branch pickers (`toolbar_pickers.rs`, a slot whose width follows
//! only the window), git actions and app buttons, in groups with even spacing
//! (`widgets::tool_gap`) that never move: the pull request button's room stays blank without a
//! hosting service. Each button's tooltip comes from `toolbar_tips.rs`, and its enabled flag
//! from the same rule in `state/toolbar_tips.rs`; the menus are in `toolbar_menus.rs`.

use super::theme::color;
use super::widgets::{self, Icon, tooltip::attach};
use crate::msg::{BranchMsg, ForgeMsg, JobMsg, Msg, PaletteMsg, PrefsMsg, PublishMsg, RepoMsg};
use crate::state::toolbar_tips as rules;
use crate::state::{AppState, PrefPage};
use eframe::egui::{self, Align, Layout};
use menus::{dropdown, terminal_button};

#[path = "toolbar_menus.rs"]
mod menus;
#[cfg(test)]
#[path = "toolbar_names_tests.rs"]
mod names_tests;
#[path = "toolbar_pickers.rs"]
mod pickers;
#[cfg(test)]
#[path = "toolbar_tests.rs"]
mod tests;
#[path = "toolbar_tips.rs"]
pub(super) mod tips;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let labels = st.settings.toolbar_labels;
    let tool = |ui: &mut egui::Ui, icon, label: &str, name: &str, enabled| widgets::tool_button(ui, icon, label, name, enabled, labels);
    egui::TopBottomPanel::top("toolbar")
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 4)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(widgets::TOOL_H);
                // repository ▾ › branch ▾ and the conflict chip, in a slot that leaves the rest room.
                pickers::pickers(ui, st, msgs);
                let Some(snap) = &st.snap else { return };
                let after_slot = ui.cursor().min.x;
                // The slot's slack already parts short names from the buttons: one group gap.
                widgets::tool_gap(ui);
                ui.spacing_mut().item_spacing.x = widgets::TOOL_SPACING;
                let (keys, idle) = (&st.keymap, st.job.is_none());
                let r = tool(ui, Icon::Undo, t.tr("toolbar.undo.button"), t.tr("toolbar.undo.name"), idle && !st.undo.undo.is_empty());
                if attach(r, keys, t, || tips::undo(st)).clicked() {
                    msgs.push(Msg::Job(JobMsg::Undo));
                }
                let r = tool(ui, Icon::Redo, t.tr("toolbar.redo.button"), t.tr("toolbar.redo.name"), idle && !st.undo.redo.is_empty());
                if attach(r, keys, t, || tips::redo(st)).clicked() {
                    msgs.push(Msg::Job(JobMsg::Redo));
                }
                widgets::tool_gap(ui);
                let r = tool(ui, Icon::Pull, t.tr("toolbar.pull.button"), t.tr("toolbar.pull.name"), rules::pull_blocked(st).is_none());
                if attach(r, keys, t, || tips::pull(st)).clicked() {
                    msgs.push(Msg::git("Pull", &["pull", "--ff"]));
                }
                dropdown(ui, "pull-menu", t.tr("toolbar.pull_menu"), 240.0, labels, |ui| {
                    let r = ui.add_enabled(rules::fetch_blocked(st).is_none(), egui::Button::new(t.tr("toolbar.fetch_all")));
                    if attach(r, keys, t, || tips::fetch_all(st)).clicked() {
                        msgs.push(Msg::git("Fetch all", &["fetch", "--all", "--prune"]));
                    }
                    // The button's text, and the job's label (English: it is also log and undo text).
                    for (text, label, a) in [
                        (t.tr("toolbar.pull_ff"), "Pull (fast-forward if possible)", &["pull", "--ff"][..]),
                        (t.tr("toolbar.pull_ff_only"), "Pull (fast-forward only)", &["pull", "--ff-only"][..]),
                        (t.tr("toolbar.pull_rebase"), "Pull (rebase)", &["pull", "--rebase"][..]),
                    ] {
                        let r = ui.add_enabled(rules::pull_variant_blocked(st, text).is_none(), egui::Button::new(text));
                        if attach(r, keys, t, || tips::pull_variant(st, text)).clicked() {
                            msgs.push(Msg::git(label, a));
                        }
                    }
                });
                let r = tool(ui, Icon::Push, t.tr("toolbar.push.button"), t.tr("toolbar.push.name"), rules::push_blocked(st).is_none());
                if attach(r, keys, t, || tips::push(st)).clicked() {
                    msgs.push(Msg::Job(JobMsg::Push { force: false }));
                }
                let host = st.forge.as_ref().filter(|f| f.host.is_some());
                dropdown(ui, "push-menu", t.tr("toolbar.push_menu"), if host.is_some() { 260.0 } else { 200.0 }, labels, |ui| {
                    if ui.add_enabled(idle, egui::Button::new(t.tr("toolbar.force_push"))).clicked() {
                        msgs.push(Msg::confirm_danger(
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
                widgets::tool_gap(ui);
                let r =
                    tool(ui, Icon::Branch, t.tr("toolbar.branch.button"), t.tr("toolbar.branch.name"), rules::branch_blocked(st).is_none());
                if attach(r, keys, t, || tips::branch(st)).clicked() {
                    msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                }
                let r = tool(ui, Icon::Stash, t.tr("toolbar.stash.button"), t.tr("toolbar.stash.name"), rules::stash_blocked(st).is_none());
                if attach(r, keys, t, || tips::stash(st)).clicked() {
                    msgs.push(Msg::git("Stash", &["stash", "push", "-u"]));
                }
                let r = tool(ui, Icon::Pop, t.tr("toolbar.pop.button"), t.tr("toolbar.pop.name"), rules::pop_blocked(st).is_none());
                if attach(r, keys, t, || tips::pop(st)).clicked() {
                    msgs.extend(crate::state::stash_confirm::pop_latest(snap));
                }
                if let Some(fs) = host {
                    let service = fs.host_provider().map(trellis_core::forge::Provider::name).unwrap_or_default();
                    let merge_request = fs.pr_noun() == "merge request";
                    let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
                    let name = if merge_request {
                        t.trf("toolbar.pr.name_merge_request", args)
                    } else {
                        t.trf("toolbar.pr.name_pull_request", args)
                    };
                    // The name starts with the caption, so speech input finds the button by what it shows.
                    let r = tool(ui, Icon::PullRequest, t.tr("toolbar.pr.button"), &name, rules::pr_blocked(st).is_none());
                    if attach(r, keys, t, || tips::pr(st, fs)).clicked()
                        && let Some(b) = &snap.head_branch
                    {
                        msgs.push(Msg::Forge(ForgeMsg::CreatePrDialog { source: b.clone(), target: None }));
                    }
                    let name = if merge_request { t.tr("toolbar.pr_menu.merge_request") } else { t.tr("toolbar.pr_menu.pull_request") };
                    dropdown(ui, "pr-menu", name, 260.0, labels, |ui| super::pr_menu_items::pr_menu(ui, snap, fs, idle, msgs));
                } else {
                    // The PR button and its caret's room, so the buttons after them stay put.
                    let w = widgets::tool_width(ui, t.tr("toolbar.pr.button"), labels);
                    ui.add_space(w + widgets::CARET_W + widgets::TOOL_SPACING);
                }
                widgets::tool_divider(ui);
                terminal_button(ui, st, labels, super::terminal::right_click_item(t, &st.settings), msgs);
                let r = tool(ui, Icon::Folder, t.tr("toolbar.folder.button"), t.tr("toolbar.folder.name"), true);
                if attach(r, keys, t, || tips::folder(st)).clicked() {
                    msgs.push(Msg::OpenFolder(snap.workdir.clone()));
                }
                let middle = ui.cursor().min.x - after_slot;

                let flips = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = tool(ui, Icon::Gear, t.tr("toolbar.settings.button"), t.tr("toolbar.settings.name"), true);
                    if attach(r, keys, t, || tips::settings(st)).clicked() {
                        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
                    }
                    let r = tool(ui, Icon::Palette, t.tr("toolbar.commands.button"), t.tr("toolbar.commands.name"), true);
                    if attach(r, keys, t, || tips::palette(st)).clicked() {
                        msgs.push(Msg::Palette(PaletteMsg::TogglePalette));
                    }
                    let r = tool(ui, Icon::Search, t.tr("toolbar.search.button"), t.tr("toolbar.search.name"), true);
                    let search = attach(r, keys, t, || tips::search(st)).clicked();
                    let r = tool(ui, Icon::Activity, t.tr("toolbar.activity.button"), t.tr("toolbar.activity.name"), true);
                    let log = attach(r, keys, t, || tips::activity(st)).clicked();
                    let r = tool(ui, Icon::Refresh, t.tr("toolbar.refresh.button"), t.tr("toolbar.refresh.name"), true);
                    if attach(r, keys, t, || tips::refresh(st)).clicked() {
                        msgs.push(Msg::Repo(RepoMsg::Refresh));
                    }
                    // The buttons' width, and the chip's with its gap when it is drawn.
                    let buttons = ui.min_rect().width();
                    widgets::tool_gap(ui);
                    let gap = ui.min_rect().width();
                    super::trust_chip::show(ui, st, msgs);
                    let right = ui.min_rect().width();
                    (search, log, if right > gap { right } else { buttons })
                });
                // View state, flipped once the tooltips above are drawn from it.
                let (search, log, right) = flips.inner;
                // The rest of the bar, with a group gap at least between the two groups.
                pickers::remember_rest(ui.ctx(), middle + widgets::GROUP_GAP + right);
                if search {
                    st.view.show_search = !st.view.show_search;
                }
                if log {
                    st.view.show_log = !st.view.show_log;
                }
            });
        });
}
