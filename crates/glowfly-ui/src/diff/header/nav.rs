//! The header's navigation group: previous and next change (or file), and the counter of where
//! the view stands.

use super::controls::{self, SPACE, ghost, icon_tip};
use crate::keymap::{BindingId, Keymap};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_header::HeaderModel;
use crate::state::diff_nav::{Counter, StepKind};
use crate::state::tip::Tip;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::widgets::Icon;
use eframe::egui::{Label, RichText, Ui};
use glowfly_core::i18n::Lang;

/// The step's description and its binding, for the next (`forward`) or previous arrow.
fn step_text(t: Lang, forward: bool, kind: StepKind) -> (BindingId, &'static str) {
    match (forward, kind) {
        (true, StepKind::Change) => (BindingId::NextChange, t.tr("diff.header.step.next")),
        (true, StepKind::File) => (BindingId::NextChange, t.tr("diff.header.step.next_file")),
        (true, StepKind::End) => (BindingId::NextChange, t.tr("diff.header.step.next_end")),
        (false, StepKind::Change) => (BindingId::PrevChange, t.tr("diff.header.step.prev")),
        (false, StepKind::File) => (BindingId::PrevChange, t.tr("diff.header.step.prev_file")),
        (false, StepKind::End) => (BindingId::PrevChange, t.tr("diff.header.step.prev_end")),
    }
}

/// The tooltip of the next (`forward`) or previous change arrow for a step that stays in the
/// file, opens another file, or goes nowhere, with the step's current key (the plain-text form of
/// the arrows' rich tooltip).
#[cfg(test)]
pub(super) fn step_tip(keys: &Keymap, t: Lang, forward: bool, kind: StepKind) -> String {
    let (id, text) = step_text(t, forward, kind);
    keys.tip(text, id)
}

fn counter_text(c: &Counter) -> String {
    format!("{} / {}", c.index, c.total)
}

fn counter_tip(t: Lang, c: &Counter) -> String {
    let args: [(&str, &dyn std::fmt::Display); 2] = [("index", &c.index), ("total", &c.total)];
    if c.files { t.trf("diff.header.counter.files.tip", &args) } else { t.trf("diff.header.counter.changes.tip", &args) }
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
    let arrows =
        [(false, Icon::PrevChange, t.tr("diff.header.prev.name"), m.prev), (true, Icon::NextChange, t.tr("diff.header.next.name"), m.next)];
    for (forward, icon, name, kind) in arrows {
        if forward {
            ui.add_space(SPACE);
        }
        let (id, text) = step_text(t, forward, kind);
        let resp = ghost(ui, ControlSize::Regular, Some(icon), None, name, kind != StepKind::End);
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
        ui.add(Label::new(text).selectable(false)).on_hover_text(counter_tip(t, &c));
    }
}
