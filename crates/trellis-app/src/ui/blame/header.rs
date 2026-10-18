//! The blame header. Row 1: "Blame", the path, the commit it is shown at, its size, and Close.
//! Row 2: Back, File history, the whitespace and move toggles, the skipped commits, and at the
//! right go to line and find in the file. Before a blame has loaded, row 2 has Back and File
//! history only.

use crate::msg::{BlameMsg, Msg};
use crate::state::blame::BlameView;
use crate::ui::common::{caret_after, short};
use crate::ui::find_bar::{FindIntent, plural};
use crate::ui::theme::color;
use eframe::egui::{self, Align, Layout, RichText, Ui};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{BlameMoves, Settings};

pub(in crate::ui) const BACK: &str = "Back";
pub(in crate::ui) const FILE_HISTORY: &str = "File history";
pub(in crate::ui) const IGNORE_WHITESPACE: &str = "Ignore whitespace";
pub(in crate::ui) const SHOW_ALL: &str = "Show all";
pub(in crate::ui) const AGE_TIP: &str = "Stronger marks in the gutter are newer commits";

/// The Moves menu's button text and its items, in order.
fn moves_text(m: BlameMoves) -> &'static str {
    match m {
        BlameMoves::Off => "Moves: Off",
        BlameMoves::WithinFile => "Moves: In file",
        BlameMoves::AcrossFiles => "Moves: Across files",
    }
}

const MOVES: [(BlameMoves, &str); 3] = [
    (BlameMoves::Off, "Off"),
    (BlameMoves::WithinFile, "Lines moved within the file"),
    (BlameMoves::AcrossFiles, "Lines moved or copied from other files changed in the same commit"),
];

/// The least width find and go to line need; narrower, they move to a row of their own.
const FIND_ROW_MIN: f32 = 340.0;

pub(super) fn show(ui: &mut Ui, blame: &mut Option<BlameView>, settings: &Settings, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) {
    let Some(view) = blame.as_mut() else { return };
    first_row(ui, view, msgs);
    ui.add_space(4.0);
    let mut own_row = false;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let back = view.back.last().map(|s| s.at.map_or_else(|| "HEAD".to_owned(), short));
        let r = ui.add_enabled(back.is_some(), egui::Button::new(BACK));
        let r = match &back {
            Some(at) => r.on_hover_text(format!("Back to the blame at {at}")),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::Blame(BlameMsg::Back));
        }
        let r = ui.button(FILE_HISTORY).on_hover_text("The history of this file from the blamed commit");
        if r.clicked() {
            msgs.push(Msg::Blame(BlameMsg::OpenHistory { select: None }));
        }
        if view.data.is_none() {
            return;
        }
        ui.separator();
        toggles(ui, settings, msgs);
        skipped_chip(ui, view, snap, msgs);
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

/// "Blame", the path, the commit it is shown at, its size, and Close.
fn first_row(ui: &mut Ui, view: &BlameView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Blame").color(color::weak()));
        ui.label(RichText::new(&view.path).strong().size(15.0).color(color::text()));
        let resolved = view.data.as_ref().map(|d| short(d.blame.at));
        let at = match (view.at, resolved) {
            (Some(oid), _) => format!("at {}", short(oid)),
            (None, Some(head)) => format!("at HEAD · {head}"),
            (None, None) => "at HEAD".to_owned(),
        };
        ui.label(RichText::new(at).color(color::weak()));
        if let Some(d) = &view.data {
            let (lines, commits) = (d.blame.lines.len(), d.blame.commits.len());
            let size = format!("{} · {}", plural(lines, "line", "lines"), plural(commits, "commit", "commits"));
            ui.label(RichText::new(size).color(color::weak())).on_hover_text(AGE_TIP);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::Blame(BlameMsg::Close));
            }
        });
    });
}

/// Ignore whitespace and the Moves menu (saved in settings).
fn toggles(ui: &mut Ui, settings: &Settings, msgs: &mut Vec<Msg>) {
    let ws = settings.blame.ignore_whitespace;
    let r = ui.selectable_label(ws, IGNORE_WHITESPACE).on_hover_text("Blame past changes that only add or remove whitespace");
    if r.clicked() {
        msgs.push(Msg::Blame(BlameMsg::SetIgnoreWhitespace(!ws)));
    }
    let moves = settings.blame.moves;
    let r = ui.menu_button(moves_text(moves), |ui| {
        for (m, label) in MOVES {
            if ui.selectable_label(moves == m, label).clicked() {
                if m != moves {
                    msgs.push(Msg::Blame(BlameMsg::SetMoves(m)));
                }
                ui.close_menu();
            }
        }
    });
    caret_after(ui, &r.response);
    ui.add_space(12.0);
}

/// "Skipping N commits" (each listed on hover) and Show all, while any are skipped.
fn skipped_chip(ui: &mut Ui, view: &BlameView, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) {
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
    let text = format!("Skipping {}", plural(view.skipped.len(), "commit", "commits"));
    ui.label(RichText::new(text).color(color::amber())).on_hover_text(tip.join("\n"));
    if ui.small_button(SHOW_ALL).on_hover_text("Stop skipping commits").clicked() {
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
