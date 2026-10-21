//! The blame header. Row 1: "Blame", the path, the commit it is shown at, its size, "Updating…"
//! while it is blamed again after HEAD moved, and Close, with the Diff | File | Blame switch
//! (Blame raised) before Close when it was opened over a file history on its shown file. Row 2:
//! Back, File history, the whitespace, move and Wrap lines toggles, the skipped commits, and at
//! the right go to line and find in the file. Before a blame has loaded, row 2 has Back and File
//! history only.

use crate::keymap::Keymap;
use crate::msg::{BlameMsg, Msg};
use crate::state::blame::BlameView;
use crate::state::history::HistoryPane;
use crate::ui::common::{caret_after, short};
use crate::ui::file_view_switch::{self, FileShow};
use crate::ui::find_bar::FindIntent;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::{self, buttons};
use eframe::egui::{Align, Layout, RichText, Ui};
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;
use trellis_core::settings::{BlameMoves, Settings};

/// The Moves menu's button text.
fn moves_text(t: Lang, m: BlameMoves) -> &'static str {
    match m {
        BlameMoves::Off => t.tr("blame.header.moves.off"),
        BlameMoves::WithinFile => t.tr("blame.header.moves.in_file"),
        BlameMoves::AcrossFiles => t.tr("blame.header.moves.across"),
    }
}

/// The Moves menu's items, in order.
fn moves(t: Lang) -> [(BlameMoves, &'static str); 3] {
    [
        (BlameMoves::Off, t.tr("blame.header.moves.item.off")),
        (BlameMoves::WithinFile, t.tr("blame.header.moves.item.within")),
        (BlameMoves::AcrossFiles, t.tr("blame.header.moves.item.across")),
    ]
}

/// The least width find and go to line need; narrower, they move to a row of their own.
const FIND_ROW_MIN: f32 = 340.0;

/// `over_history`: opened over a file history on its shown file, which is still open.
pub(super) fn show(
    ui: &mut Ui,
    blame: &mut Option<BlameView>,
    settings: &Settings,
    snap: Option<&Snapshot>,
    over_history: bool,
    keys: &Keymap,
    msgs: &mut Vec<Msg>,
) {
    let Some(view) = blame.as_mut() else { return };
    let t = settings.lang();
    first_row(ui, t, keys, view, over_history, msgs);
    ui.add_space(4.0);
    let mut own_row = false;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let back = view.back.last().map(|s| s.at.map_or_else(|| "HEAD".to_owned(), short));
        let r = ui.add(buttons::secondary(t.tr("blame.header.back.button")).enabled(back.is_some()));
        let r = match &back {
            Some(at) => r.on_hover_text(t.trf("blame.header.back.tip", &[("at", at)])),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::Blame(BlameMsg::Back));
        }
        let r = ui.add(buttons::secondary(t.tr("blame.header.file_history.button"))).on_hover_text(t.tr("blame.header.file_history.tip"));
        if r.clicked() {
            msgs.push(Msg::Blame(BlameMsg::OpenHistory { select: None }));
        }
        if view.data.is_none() {
            return;
        }
        ui.separator();
        toggles(ui, t, settings, msgs);
        skipped_chip(ui, t, view, snap, msgs);
        if ui.available_width() < FIND_ROW_MIN {
            own_row = true;
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            find_and_line(ui, view, msgs);
        });
    });
    if own_row {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                find_and_line(ui, view, msgs);
            });
        });
    }
}

/// "Blame", the path, the commit it is shown at, its size, and Close (after the Diff | File |
/// Blame switch back to the history, `over_history`).
fn first_row(ui: &mut Ui, t: Lang, keys: &Keymap, view: &BlameView, over_history: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(t.tr("blame.header.title")).color(color::weak()));
        ui.label(RichText::new(&view.path).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()));
        let resolved = view.data.as_ref().map(|d| short(d.blame.at));
        let at = match (view.at, resolved) {
            (Some(oid), _) => t.trf("blame.header.at", &[("sha", &short(oid))]),
            (None, Some(head)) => t.trf("blame.header.at_head_sha", &[("sha", &head)]),
            (None, None) => t.tr("blame.header.at_head").to_owned(),
        };
        ui.label(RichText::new(at).color(color::weak()));
        if let Some(d) = &view.data {
            let (lines, commits) = (d.blame.lines.len(), d.blame.commits.len());
            let (lines, commits) = (t.trn("blame.header.lines", lines, &[]), t.trn("blame.header.commits", commits, &[]));
            let size = t.trf("blame.header.size", &[("lines", &lines), ("commits", &commits)]);
            ui.label(RichText::new(size).color(color::weak())).on_hover_text(t.tr("blame.header.age.tip"));
        }
        if view.updating() {
            let updating = RichText::new(t.tr("blame.header.updating")).color(color::weak());
            ui.label(updating).on_hover_text(t.tr("blame.header.updating.tip"));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::ghost::close_view(ui, keys, t, t.tr("blame.header.close.name")).clicked() {
                msgs.push(Msg::Blame(BlameMsg::Close));
            }
            if over_history {
                ui.add_space(8.0);
                let pane = match file_view_switch::show(ui, FileShow::Blame, None, None) {
                    Some(FileShow::Diff) => Some(HistoryPane::Diff),
                    Some(FileShow::File) => Some(HistoryPane::File),
                    Some(FileShow::Blame) | None => None,
                };
                if let Some(pane) = pane {
                    msgs.push(Msg::Blame(BlameMsg::BackToHistory(pane)));
                }
            }
        });
    });
}

/// Ignore whitespace, the Moves menu and Wrap lines (saved in settings).
fn toggles(ui: &mut Ui, t: Lang, settings: &Settings, msgs: &mut Vec<Msg>) {
    let ws = settings.blame.ignore_whitespace;
    let r = ui.selectable_label(ws, t.tr("blame.header.ignore_ws")).on_hover_text(t.tr("blame.header.ignore_ws.tip"));
    if r.clicked() {
        msgs.push(Msg::Blame(BlameMsg::SetIgnoreWhitespace(!ws)));
    }
    let moves = settings.blame.moves;
    let r = ui.menu_button(moves_text(t, moves), |ui| {
        for (m, label) in self::moves(t) {
            if ui.selectable_label(moves == m, label).clicked() {
                if m != moves {
                    msgs.push(Msg::Blame(BlameMsg::SetMoves(m)));
                }
                ui.close_menu();
            }
        }
    });
    caret_after(ui, &r.response);
    ui.add_space(12.0); // clear of the caret
    let wrap = settings.blame.wrap_lines;
    if ui.selectable_label(wrap, t.tr("blame.header.wrap")).on_hover_text(t.tr("blame.header.wrap.tip")).clicked() {
        msgs.push(Msg::Blame(BlameMsg::SetWrapLines(!wrap)));
    }
    ui.add_space(12.0);
}

/// "Skipping N commits" (each listed on hover) and Show all, while any are skipped.
fn skipped_chip(ui: &mut Ui, t: Lang, view: &BlameView, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) {
    if view.skipped.is_empty() {
        return;
    }
    let summary = |oid| snap.and_then(|s| s.row_of.get(&oid).and_then(|&r| s.commits.get(r))).map(|c| c.summary.as_str());
    let tip: Vec<String> = view
        .skipped
        .iter()
        .map(|&oid| match summary(oid) {
            Some(s) => format!("{} {s}", short(oid)),
            None => short(oid),
        })
        .collect();
    let text = t.trn("blame.header.skipping", view.skipped.len(), &[]);
    ui.label(RichText::new(text).color(color::warn())).on_hover_text(tip.join("\n"));
    let show_all = buttons::secondary(t.tr("blame.header.show_all.button")).small();
    if ui.add(show_all).on_hover_text(t.tr("blame.header.show_all.tip")).clicked() {
        msgs.push(Msg::Blame(BlameMsg::ClearSkipped));
    }
}

/// Right to left: the Line field, the find arrows, the match count and the Find field.
fn find_and_line(ui: &mut Ui, view: &mut BlameView, msgs: &mut Vec<Msg>) {
    for intent in crate::ui::find_bar::find_and_line(ui, &mut view.find, &mut view.goto, &mut view.focus_find) {
        msgs.push(Msg::Blame(match intent {
            FindIntent::Changed => BlameMsg::FindChanged,
            FindIntent::Step(d) => BlameMsg::FindStep(d),
            FindIntent::GoToLine => BlameMsg::GoToLine,
        }));
    }
}
