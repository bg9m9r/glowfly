//! Top toolbar, on the side panels' surface so the active tab runs into it: the repository ›
//! branch pickers (`toolbar_pickers.rs`, in fixed slots so the bar never moves when the names
//! change), then the git actions and app buttons, each group on its own recessed surface
//! (`widgets::tool_group`) with even gaps between them (`widgets::tool_gap`). The middle groups
//! follow the pickers, the pull request group shows only with a hosting service, and the
//! right-hand group is anchored to the window's edge with the untrusted and conflict chips
//! before it, so a chip appearing moves nothing else. Each button's tooltip comes from
//! `toolbar_tips.rs`, and its enabled flag from the same rule in `state/toolbar_tips.rs`; the
//! menus are in `toolbar_menus.rs`.

use super::backdrop::{self, Surface};
use super::theme::color;
use super::widgets::reason::{self, ItemState};
use super::widgets::{self, Icon, tooltip::attach};
use crate::msg::{BranchMsg, ForgeMsg, JobMsg, Msg, PaletteMsg, PrefsMsg, PublishMsg, RepoMsg};
use crate::state::toolbar_tips as rules;
use crate::state::{AppState, PrefPage};
use crate::ui::widgets::menu::separator;
use eframe::egui::{self, Align, Layout};
use menus::{split, terminal_button};

#[cfg(test)]
#[path = "toolbar_look_tests.rs"]
mod look_tests;
#[path = "toolbar_menus.rs"]
mod menus;
#[cfg(test)]
#[path = "toolbar_names_tests.rs"]
mod names_tests;
#[path = "toolbar_pickers.rs"]
mod pickers;
#[cfg(test)]
#[path = "toolbar_still_tests.rs"]
mod still_tests;
#[cfg(test)]
#[path = "toolbar_tests.rs"]
mod tests;
#[path = "toolbar_tips.rs"]
pub(super) mod tips;

/// Between the untrusted chip and the conflict chip.
const CHIP_GAP: f32 = 8.0;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let labels = st.settings.toolbar_labels;
    let tool = |ui: &mut egui::Ui, icon, label: &str, name: &str, enabled| widgets::tool_button(ui, icon, label, name, enabled, labels);
    egui::TopBottomPanel::top("toolbar")
        .frame(
            egui::Frame::new()
                .fill(backdrop::fill(st.view.backdrop, Surface::Side, color::panel()))
                .inner_margin(egui::Margin::symmetric(12, 4)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(widgets::GROUP_H);
                // [repository › branch]
                pickers::pickers(ui, st, msgs);
                let Some(snap) = &st.snap else { return };
                let after_pickers = ui.cursor().min.x;
                let (keys, idle) = (&st.keymap, st.job.is_none());
                widgets::tool_gap(ui);
                widgets::tool_group(ui, |ui| {
                    let r = tool(
                        ui,
                        Icon::Undo,
                        t.tr("toolbar.undo.button"),
                        t.tr("toolbar.undo.name"),
                        idle && !st.session.undo.undo.is_empty(),
                    );
                    if attach(r, keys, t, || tips::undo(st)).clicked() {
                        msgs.push(Msg::Job(JobMsg::Undo));
                    }
                    let r = tool(
                        ui,
                        Icon::Redo,
                        t.tr("toolbar.redo.button"),
                        t.tr("toolbar.redo.name"),
                        idle && !st.session.undo.redo.is_empty(),
                    );
                    if attach(r, keys, t, || tips::redo(st)).clicked() {
                        msgs.push(Msg::Job(JobMsg::Redo));
                    }
                });
                widgets::tool_gap(ui);
                let host = st.session.forge.as_ref().filter(|f| f.host.is_some());
                widgets::tool_group(ui, |ui| {
                    let pull = (Icon::Pull, t.tr("toolbar.pull.button"), t.tr("toolbar.pull.name"));
                    // The button's text, and the job's label (English: it is also log and undo text).
                    let variants = [
                        (t.tr("toolbar.pull_ff"), "Pull (fast-forward if possible)", &["pull", "--ff"][..]),
                        (t.tr("toolbar.pull_ff_only"), "Pull (fast-forward only)", &["pull", "--ff-only"][..]),
                        (t.tr("toolbar.pull_rebase"), "Pull (rebase)", &["pull", "--rebase"][..]),
                    ];
                    let live =
                        rules::fetch_blocked(st).is_none() || variants.iter().any(|v| rules::pull_variant_blocked(st, v.0).is_none());
                    let menu = (t.tr("toolbar.pull_menu"), 240.0, live);
                    let r = split(ui, "pull-menu", pull, rules::pull_blocked(st).is_none(), labels, menu, |ui| {
                        // A blocked entry shows its reason; a ready one, what it would do.
                        let why = rules::fetch_blocked(st);
                        let r = reason::menu_button(ui, t.tr("toolbar.fetch_all"), "", ItemState::blocked_by(why.as_deref()));
                        let r = if why.is_none() { attach(r, keys, t, || tips::fetch_all(st)) } else { r };
                        if r.clicked() {
                            msgs.push(Msg::git("Fetch all", &["fetch", "--all", "--prune"]));
                        }
                        for (text, label, a) in variants {
                            let why = rules::pull_variant_blocked(st, text);
                            let r = reason::menu_button(ui, text, "", ItemState::blocked_by(why.as_deref()));
                            let r = if why.is_none() { attach(r, keys, t, || tips::pull_variant(st, text)) } else { r };
                            if r.clicked() {
                                msgs.push(Msg::git(label, a));
                            }
                        }
                    });
                    if attach(r, keys, t, || tips::pull(st)).clicked() {
                        msgs.push(Msg::git("Pull", &["pull", "--ff"]));
                    }
                    let push = (Icon::Push, t.tr("toolbar.push.button"), t.tr("toolbar.push.name"));
                    let menu = (t.tr("toolbar.push_menu"), if host.is_some() { 260.0 } else { 200.0 }, rules::busy_blocked(st).is_none());
                    let r = split(ui, "push-menu", push, rules::push_blocked(st).is_none(), labels, menu, |ui| {
                        let busy = rules::busy_blocked(st);
                        let busy = ItemState::blocked_by(busy.as_deref());
                        if reason::menu_button(ui, t.tr("toolbar.force_push"), "", busy).clicked() {
                            msgs.push(Msg::confirm_danger(
                                t.tr("toolbar.force_push.title"),
                                t.tr("toolbar.force_push.body").into(),
                                t.tr("toolbar.force_push.button"),
                                Msg::Job(JobMsg::Push { force: true }),
                            ));
                        }
                        if snap.remotes.is_empty() && reason::menu_button(ui, t.tr("toolbar.publish"), "", busy).clicked() {
                            msgs.push(Msg::Publish(PublishMsg::PublishDialog));
                        }
                        if let Some(fs) = host {
                            separator(ui);
                            super::pr_menu_items::push_pr_item(ui, t, snap, fs, idle, msgs);
                        }
                    });
                    if attach(r, keys, t, || tips::push(st)).clicked() {
                        msgs.push(Msg::Job(JobMsg::Push { force: false }));
                    }
                });
                widgets::tool_gap(ui);
                widgets::tool_group(ui, |ui| {
                    let enabled = rules::branch_blocked(st).is_none();
                    let r = tool(ui, Icon::Branch, t.tr("toolbar.branch.button"), t.tr("toolbar.branch.name"), enabled);
                    if attach(r, keys, t, || tips::branch(st)).clicked() {
                        msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                    }
                    let r =
                        tool(ui, Icon::Stash, t.tr("toolbar.stash.button"), t.tr("toolbar.stash.name"), rules::stash_blocked(st).is_none());
                    if attach(r, keys, t, || tips::stash(st)).clicked() {
                        msgs.push(Msg::git("Stash", &["stash", "push", "-u"]));
                    }
                    let r = tool(ui, Icon::Pop, t.tr("toolbar.pop.button"), t.tr("toolbar.pop.name"), rules::pop_blocked(st).is_none());
                    if attach(r, keys, t, || tips::pop(st)).clicked() {
                        msgs.extend(crate::state::stash_confirm::pop_latest(snap));
                    }
                });
                if let Some(fs) = host {
                    widgets::tool_gap(ui);
                    widgets::tool_group(ui, |ui| {
                        let service = fs.host_provider().map(glowfly_core::forge::Provider::name).unwrap_or_default();
                        let merge_request = fs.pr_noun() == "merge request";
                        let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
                        let name = if merge_request {
                            t.trf("toolbar.pr.name_merge_request", args)
                        } else {
                            t.trf("toolbar.pr.name_pull_request", args)
                        };
                        // The name starts with the caption, so speech input finds the button by what it shows.
                        let pr = (Icon::PullRequest, t.tr("toolbar.pr.button"), name.as_str());
                        let menu_name =
                            if merge_request { t.tr("toolbar.pr_menu.merge_request") } else { t.tr("toolbar.pr_menu.pull_request") };
                        let menu = (menu_name, 260.0, super::pr_menu_items::pr_menu_live(snap, fs, idle));
                        let r = split(ui, "pr-menu", pr, rules::pr_blocked(st).is_none(), labels, menu, |ui| {
                            super::pr_menu_items::pr_menu(ui, t, snap, fs, idle, msgs);
                        });
                        if attach(r, keys, t, || tips::pr(st, fs)).clicked()
                            && let Some(b) = &snap.head_branch
                        {
                            msgs.push(Msg::Forge(ForgeMsg::CreatePrDialog { source: b.clone(), target: None }));
                        }
                    });
                }
                widgets::tool_gap(ui);
                widgets::tool_group(ui, |ui| {
                    terminal_button(ui, st, labels, super::terminal::right_click_item(t, &st.settings), msgs);
                    let r = tool(ui, Icon::Folder, t.tr("toolbar.folder.button"), t.tr("toolbar.folder.name"), true);
                    if attach(r, keys, t, || tips::folder(st)).clicked() {
                        msgs.push(Msg::OpenFolder(snap.workdir.clone()));
                    }
                });
                let middle = ui.cursor().min.x - after_pickers;

                let flips = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let group = widgets::tool_group(ui, |ui| {
                        let r = tool(ui, Icon::Gear, t.tr("toolbar.settings.button"), t.tr("toolbar.settings.name"), true);
                        if attach(r, keys, t, || tips::settings(st)).clicked() {
                            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
                        }
                        let r = tool(ui, Icon::Palette, t.tr("toolbar.commands.button"), t.tr("toolbar.commands.name"), true);
                        if attach(r, keys, t, || tips::palette(st)).clicked() {
                            msgs.push(Msg::Palette(PaletteMsg::TogglePalette));
                        }
                        let toggle =
                            |ui: &mut egui::Ui, icon, label: &str, name: &str, on| widgets::tool_toggle(ui, icon, label, name, on, labels);
                        let r = toggle(ui, Icon::Search, t.tr("toolbar.search.button"), t.tr("toolbar.search.name"), st.view.show_search);
                        let search = attach(r, keys, t, || tips::search(st)).clicked();
                        let r =
                            toggle(ui, Icon::Activity, t.tr("toolbar.activity.button"), t.tr("toolbar.activity.name"), st.view.show_log);
                        let log = attach(r, keys, t, || tips::activity(st)).clicked();
                        let r = tool(ui, Icon::Refresh, t.tr("toolbar.refresh.button"), t.tr("toolbar.refresh.name"), true);
                        if attach(r, keys, t, || tips::refresh(st)).clicked() {
                            msgs.push(Msg::Repo(RepoMsg::Refresh));
                        }
                        (search, log)
                    });
                    let (search, log) = group.inner;
                    // The group's width, and the chips' with their gaps when they are drawn.
                    let buttons = ui.min_rect().width();
                    widgets::tool_gap(ui);
                    let gap = ui.min_rect().width();
                    super::trust_chip::show(ui, st, msgs);
                    if ui.min_rect().width() > gap {
                        ui.add_space(CHIP_GAP);
                    }
                    super::conflict_check::indicator(ui, st, msgs);
                    let right = ui.min_rect().width();
                    (search, log, if right > gap { right } else { buttons })
                });
                // View state, flipped once the tooltips above are drawn from it.
                let (search, log, right) = flips.inner;
                // Everything after the pickers, with a group gap at least before the right-hand group.
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
