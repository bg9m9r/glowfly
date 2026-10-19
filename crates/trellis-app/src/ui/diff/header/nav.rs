//! The header's navigation group: previous and next change (or file), and the counter of where
//! the view stands.

use super::controls::{self, SPACE, ghost, icon_tip};
use crate::keymap::{BindingId, Keymap};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_header::HeaderModel;
use crate::state::diff_nav::{Counter, StepKind};
use crate::state::tip::Tip;
use crate::ui::theme::color;
use crate::ui::widgets::Icon;
use eframe::egui::{Label, RichText, Ui};
use trellis_core::i18n::Lang;

pub(super) const PREV_NAME: &str = "Previous change";
pub(super) const NEXT_NAME: &str = "Next change";

/// The step's description and its binding, for the next (`forward`) or previous arrow.
fn step_text(forward: bool, kind: StepKind) -> (BindingId, &'static str) {
    match (forward, kind) {
        (true, StepKind::Change) => (BindingId::NextChange, "Next change"),
        (true, StepKind::File) => (BindingId::NextChange, "Next change, in the next file"),
        (true, StepKind::End) => (BindingId::NextChange, "No more changes after this file"),
        (false, StepKind::Change) => (BindingId::PrevChange, "Previous change"),
        (false, StepKind::File) => (BindingId::PrevChange, "Previous change, in the previous file"),
        (false, StepKind::End) => (BindingId::PrevChange, "No changes before this file"),
    }
}

/// The tooltip of the next (`forward`) or previous change arrow for a step that stays in the
/// file, opens another file, or goes nowhere, with the step's current key (the plain-text form of
/// the arrows' rich tooltip).
#[cfg(test)]
pub(super) fn step_tip(keys: &Keymap, forward: bool, kind: StepKind) -> String {
    let (id, text) = step_text(forward, kind);
    keys.tip(text, id)
}

fn counter_text(c: &Counter) -> String {
    format!("{} / {}", c.index, c.total)
}

fn counter_tip(c: &Counter) -> String {
    if c.files { format!("File {} of {} in this list", c.index, c.total) } else { format!("Change {} of {}", c.index, c.total) }
}

/// The arrows' width, and the counter's with the space before it (0 without one); both 0 when
/// the group doesn't show.
pub(super) fn widths(ui: &Ui, m: &HeaderModel) -> (f32, f32) {
    if !m.show_nav {
        return (0.0, 0.0);
    }
    let counter = m.counter.map_or(0.0, |c| SPACE + controls::text_w(ui, &counter_text(&c), controls::font()));
    (2.0 * controls::HEIGHT + SPACE, counter)
}

/// Draws the arrows and, with `counter`, the counter.
pub(super) fn show(ui: &mut Ui, m: &HeaderModel, counter: bool, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    for (forward, icon, name, kind) in [(false, Icon::PrevChange, PREV_NAME, m.prev), (true, Icon::NextChange, NEXT_NAME, m.next)] {
        if forward {
            ui.add_space(SPACE);
        }
        let (id, text) = step_text(forward, kind);
        let resp = ghost(ui, Some(icon), None, name, kind != StepKind::End);
        let resp = icon_tip(resp, keys, t, Tip::new(text).keys(id));
        if resp.clicked() {
            msgs.push(Msg::Diff(DiffMsg::JumpToChange(if forward { 1 } else { -1 })));
            // Leave the arrow keys to the diff.
            resp.surrender_focus();
        }
    }
    if let Some(c) = m.counter.filter(|_| counter) {
        ui.add_space(SPACE);
        let text = RichText::new(counter_text(&c)).font(controls::font()).color(color::weak());
        ui.add(Label::new(text).selectable(false)).on_hover_text(counter_tip(&c));
    }
}
