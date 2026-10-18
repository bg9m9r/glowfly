//! Find in a file and go to a line, as the blame header and the file history's File view show
//! them: laid out right to left, the Line field, the next and previous match arrows, the match
//! count and the Find field.

use crate::state::blame_find::BlameFind;
use crate::ui::access::name_field;
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, small_icon};
use eframe::egui::{self, Key, RichText, Ui};

pub(in crate::ui) const FIND_HINT: &str = "Find in file";
pub(in crate::ui) const LINE_HINT: &str = "Line";

/// What the controls asked for this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum FindIntent {
    /// The Find field's text changed.
    Changed,
    /// To the next (1) or previous (-1) match.
    Step(isize),
    /// Enter in the Line field.
    GoToLine,
}

pub(in crate::ui) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Draws the controls in a right-to-left layout and returns what they asked for, in order.
/// `focus` focuses the Find field (and is cleared).
pub(in crate::ui) fn find_and_line(ui: &mut Ui, find: &mut BlameFind, goto: &mut String, focus: &mut bool) -> Vec<FindIntent> {
    let mut out = Vec::new();
    let line = ui.add(egui::TextEdit::singleline(goto).hint_text(LINE_HINT).desired_width(70.0));
    name_field(&line, LINE_HINT);
    if line.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        out.push(FindIntent::GoToLine);
    }
    let hits = find.hits.len();
    let arrow = if hits > 0 { color::text() } else { color::faint() };
    if small_icon(ui, Icon::NextChange, "Next match (Enter)", arrow).clicked() && hits > 0 {
        out.push(FindIntent::Step(1));
    }
    if small_icon(ui, Icon::PrevChange, "Previous match (Shift+Enter)", arrow).clicked() && hits > 0 {
        out.push(FindIntent::Step(-1));
    }
    if !find.query.is_empty() {
        let count = match (hits, find.current) {
            (0, _) => "No matches".to_owned(),
            (n, Some(k)) => format!("{} of {n}", k + 1),
            (n, None) => plural(n, "match", "matches"),
        };
        ui.label(RichText::new(count).color(color::weak()));
    }
    let w = (ui.available_width() - 16.0).clamp(80.0, 180.0);
    let field = ui.add(egui::TextEdit::singleline(&mut find.query).hint_text(FIND_HINT).desired_width(w));
    name_field(&field, FIND_HINT);
    if std::mem::take(focus) {
        field.request_focus();
    }
    if field.changed() {
        out.push(FindIntent::Changed);
    }
    if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.input(|i| i.modifiers.shift);
        out.push(FindIntent::Step(if back { -1 } else { 1 }));
        field.request_focus();
    }
    out
}
