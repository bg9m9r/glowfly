//! An exec or break row of the rebase panel: a grip, a chip naming the step, the command (or a
//! note for a break), and Edit command, Remove step, Move up and Move down.

use super::commit_row::{line, move_buttons, paint_back, paint_chip};
use super::rows::{BUTTONS_W, Place, SHA_X, Shade};
use crate::msg::{Msg, RebaseMsg};
use crate::ui::theme::color;
use crate::ui::widgets::Icon;
use eframe::egui::{FontId, Rect, Ui, pos2};
use trellis_core::rebase::TodoStep;

pub const BREAK_NOTE: &str = "Stop here; Continue goes on";
pub const NO_COMMAND: &str = "(no command yet)";

/// The row's background, chip and text.
pub(super) fn paint_step(ui: &Ui, rect: Rect, step: &TodoStep, shade: Shade) {
    paint_back(ui, rect, shade);
    let cy = rect.center().y;
    let x = rect.left() + SHA_X;
    let max_w = rect.right() - BUTTONS_W - 16.0 - x;
    match step {
        TodoStep::Exec(cmd) => {
            paint_chip(ui, rect, "exec", color::primary());
            if cmd.trim().is_empty() {
                line(ui, pos2(x, cy), NO_COMMAND, FontId::proportional(13.0), color::faint(), max_w);
            } else {
                line(ui, pos2(x, cy), cmd, FontId::monospace(12.5), color::text(), max_w);
            }
        }
        TodoStep::Break => {
            paint_chip(ui, rect, "break", color::amber());
            line(ui, pos2(x, cy), BREAK_NOTE, FontId::proportional(13.0), color::weak(), max_w);
        }
        TodoStep::Commit(_) => {}
    }
}

/// Edit command (exec steps), Remove step, Move up and Move down.
pub(super) fn step_controls(ui: &mut Ui, rect: Rect, at: Place, step: &TodoStep, msgs: &mut Vec<Msg>) {
    let edit = matches!(step, TodoStep::Exec(_)).then(|| (Icon::Pencil, "Edit command", RebaseMsg::RebaseEditMessage(at.i)));
    move_buttons(ui, rect, at, edit, Some(RebaseMsg::RebaseRemoveStep(at.i)), msgs);
}
