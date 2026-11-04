//! Top toolbar, on the side panels' surface so the active tab runs into it: the repository ›
//! branch pickers (`toolbar_pickers.rs`, one framed segment bar fitted to the names, steady
//! across checkouts: `toolbar_fit.rs`), then the git actions and app buttons in borderless groups of
//! quiet buttons (`widgets::tool_group`), each set apart by a divider (`widgets::tool_gap`). The middle groups
//! (`toolbar_groups.rs`) follow the pickers, the pull request group shows only with a hosting
//! service, and the right-hand group is anchored to the window's edge with the untrusted and
//! conflict chips before it, so a chip appearing moves nothing else. In a narrow window the
//! parts that don't fit fold into the More menu (`toolbar_fold.rs`, `toolbar_more.rs`); a chip
//! counts in what must fit, so it folds a group rather than squeezing the pickers. Each
//! button's tooltip comes from `toolbar_tips.rs`, and its enabled flag from the same rule in
//! `state/toolbar_tips.rs`; the menus are in `toolbar_menus.rs`.

use super::backdrop::{self, Surface};
use super::theme::color;
use super::widgets::{self, Icon, tooltip::attach};
use eframe::egui::{self, Align, Id, Layout};
use fold::{ORDER, Part, is_folded};
use glowfly_model::msg::{Msg, PaletteMsg, PrefsMsg, RepoMsg};
use glowfly_model::state::{AppState, PrefPage, ViewState};

#[path = "toolbar_fit.rs"]
mod fit;
#[path = "toolbar_fold.rs"]
mod fold;
#[path = "toolbar_groups.rs"]
mod groups;
#[cfg(test)]
#[path = "toolbar_look_tests.rs"]
mod look_tests;
#[path = "toolbar_menus.rs"]
mod menus;
#[path = "toolbar_more.rs"]
mod more;
#[cfg(test)]
#[path = "toolbar_names_tests.rs"]
mod names_tests;
#[cfg(test)]
#[path = "toolbar_narrow_tests.rs"]
mod narrow_tests;
#[path = "toolbar_pickers.rs"]
mod pickers;
#[cfg(test)]
#[path = "toolbar_still_tests.rs"]
mod still_tests;
#[cfg(test)]
#[path = "toolbar_sync_tests.rs"]
mod sync_tests;
#[cfg(test)]
#[path = "toolbar_tests.rs"]
mod tests;
#[path = "toolbar_tips.rs"]
pub(super) mod tips;

/// Between the untrusted chip and the conflict chip.
const CHIP_GAP: f32 = 8.0;
/// Where the toolbar keeps the chips' width with their gaps, measured each frame.
const CHIPS: &str = "toolbar-chips";

pub fn show(ctx: &egui::Context, st: &AppState, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let labels = st.settings.toolbar_labels;
    let tool = |ui: &mut egui::Ui, icon, label: &str, name: &str, enabled| widgets::tool_button(ui, icon, label, name, enabled, labels);
    egui::TopBottomPanel::top("toolbar").frame(egui::Frame::new().fill(ground(st)).inner_margin(egui::Margin::symmetric(12, 4))).show(
        ctx,
        |ui| {
            ui.horizontal(|ui| {
                ui.set_height(widgets::GROUP_H);
                let room = ui.available_width();
                // [repository › branch]
                let slots = pickers::pickers(ui, st, msgs);
                let Some(snap) = &st.snap else { return };
                let host = st.session.forge.as_ref().filter(|f| f.host.is_some());
                let chips = ui.ctx().data(|d| d.get_temp::<f32>(Id::new(CHIPS))).unwrap_or(0.0);
                let k = fold::folded(&widths(ui, st, (host.is_some(), slots), room, chips));
                let shown = |part| !is_folded(part, k);
                let after_pickers = ui.cursor().min.x;
                let keys = &st.keymap;
                if shown(Part::Undo) {
                    widgets::tool_gap(ui);
                    groups::undo(ui, st, msgs);
                }
                widgets::tool_gap(ui);
                groups::sync(ui, st, snap, host, msgs);
                if shown(Part::Branch) {
                    widgets::tool_gap(ui);
                    groups::branch(ui, st, snap, msgs);
                }
                if let Some(fs) = host.filter(|_| shown(Part::Pr)) {
                    widgets::tool_gap(ui);
                    groups::pr(ui, st, snap, fs, msgs);
                }
                if shown(Part::Files) {
                    widgets::tool_gap(ui);
                    groups::files(ui, st, snap, msgs);
                }
                // A pull request group that isn't there folds without needing the menu.
                if ORDER[..k].iter().any(|&p| p != Part::Pr || host.is_some()) {
                    widgets::tool_gap(ui);
                    more::show(ui, st, snap, host, k, msgs);
                }
                let middle = ui.cursor().min.x - after_pickers;

                let flips = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let group = widgets::tool_group(ui, |ui| {
                        if shown(Part::Settings) {
                            let r = tool(ui, Icon::Gear, t.tr("toolbar.settings.button"), t.tr("toolbar.settings.name"), true);
                            if attach(r, keys, t, || tips::settings(st)).clicked() {
                                msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
                            }
                        }
                        if shown(Part::Commands) {
                            let r = tool(ui, Icon::Palette, t.tr("toolbar.commands.button"), t.tr("toolbar.commands.name"), true);
                            if attach(r, keys, t, || tips::palette(st)).clicked() {
                                msgs.push(Msg::Palette(PaletteMsg::TogglePalette));
                            }
                        }
                        let toggle =
                            |ui: &mut egui::Ui, icon, label: &str, name: &str, on| widgets::tool_toggle(ui, icon, label, name, on, labels);
                        let r = toggle(ui, Icon::Search, t.tr("toolbar.search.button"), t.tr("toolbar.search.name"), view.show_search);
                        let search = attach(r, keys, t, || tips::search(st)).clicked();
                        let r = toggle(ui, Icon::Activity, t.tr("toolbar.activity.button"), t.tr("toolbar.activity.name"), view.show_log);
                        let log = attach(r, keys, t, || tips::activity(st, view.show_log)).clicked();
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
                    (search, log, if right > gap { right } else { buttons }, buttons)
                });
                // View state, flipped once the tooltips above are drawn from it.
                let (search, log, right, buttons) = flips.inner;
                remember(ui.ctx(), CHIPS, right - buttons);
                // Everything after the pickers, with a group gap at least before the right-hand group.
                pickers::remember_rest(ui.ctx(), middle + widgets::GROUP_GAP + right);
                if search {
                    view.show_search = !view.show_search;
                }
                if log {
                    view.show_log = !view.show_log;
                }
            });
        },
    );
}

/// The toolbar's fill: the side panels' surface, which the active tab runs into.
fn ground(st: &AppState) -> egui::Color32 {
    backdrop::fill(st.cues.backdrop, Surface::Side, color::panel())
}

/// Keeps `value` under `key` for the next frame, drawing once more when it changed.
fn remember(ctx: &egui::Context, key: &str, value: f32) {
    let old = ctx.data_mut(|d| std::mem::replace(d.get_temp_mut_or(Id::new(key), 0.0), value));
    if (old - value).abs() >= 0.5 {
        ctx.request_repaint();
    }
}

/// The widths the fold decision needs (`toolbar_fold::Widths`), in a toolbar row `room` wide
/// with the chips and their gaps taking `chips`, `host` telling whether the pull request group
/// shows and `slots` the pickers' fitted name slots.
fn widths(ui: &egui::Ui, st: &AppState, (host, slots): (bool, (f32, f32)), room: f32, chips: f32) -> fold::Widths {
    let t = st.settings.lang();
    let slot = |key: &'static str, split: bool| groups::slot(ui, st, t.tr(key), split);
    let plain = |keys: &[&'static str]| groups::group_width(&keys.iter().map(|k| slot(k, false)).collect::<Vec<_>>());
    let gap = widgets::GROUP_GAP;
    // Pull's caret is followed by Push: `SPLIT_GAP` between them, not the group's spacing.
    let split_gap = widgets::SPLIT_GAP - widgets::TOOL_SPACING;
    let badged = |key: &'static str| widgets::badged_tool_width(ui, t.tr(key), st.settings.toolbar_labels) + widgets::CARET_W;
    let sync = groups::group_width(&[badged("toolbar.pull.button"), badged("toolbar.push.button")]) + split_gap;
    let right = plain(&["toolbar.search.button", "toolbar.activity.button", "toolbar.refresh.button"]);
    // PR or MR, by the host: room for the wider.
    let caption = slot("toolbar.pr.button", true).max(slot("toolbar.pr.button_merge_request", true));
    let pr = if host { gap + groups::group_width(&[caption]) } else { 0.0 };
    let in_right = |key| slot(key, false) + widgets::TOOL_SPACING;
    let parts = [
        gap + plain(&["toolbar.terminal.button", "toolbar.folder.button"]),
        gap + plain(&["toolbar.undo.button", "toolbar.redo.button"]),
        gap + plain(&["toolbar.branch.button", "toolbar.stash.button", "toolbar.pop.button"]),
        pr,
        in_right("toolbar.commands.button"),
        in_right("toolbar.settings.button"),
    ];
    let more = gap + groups::group_width(&[groups::slot(ui, st, more::caption(st), false)]);
    // The pickers' room counts the spacing after their group, as `toolbar_pickers` does.
    let pickers = pickers::comfort(slots) + ui.spacing().item_spacing.x;
    fold::Widths { room, pickers, fixed: 2.0 * gap + sync + right + chips, more, parts }
}
