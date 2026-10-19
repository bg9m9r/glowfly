//! The Choose the mainline parent dialog (`Modal::Mainline`): which parent of a merge commit a
//! cherry-pick or revert counts the merge's changes against.

use super::common::short;
use super::dialog_frame::{Cx, Dialog, Footer};
use super::widgets::buttons;
use crate::msg::{Msg, SelectionMsg};
use crate::state::pick::{MainlineForm, PickOp};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use eframe::egui::{self, RichText};

/// What the dialog explains about the choice.
const EXPLAIN: &str = "A merge commit has more than one parent. The change applied is what the merge brought in compared with the \
                       parent you choose, which is usually parent 1, the branch that was merged into.";

/// Draws the dialog; returns true when it should close (submitted or Cancel). The primary button
/// waits while a job runs (`idle` false).
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut MainlineForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let what = match form.op {
        PickOp::CherryPick => "Cherry-pick",
        PickOp::Revert => "Revert",
    };
    let d = Dialog::new(cx, "Choose the mainline parent").begin(ui);
    ui.label(RichText::new(format!("{what} merge commit {} {}", short(form.oid), form.summary)).color(color::text()));
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(EXPLAIN).color(color::weak())).wrap());
    ui.add_space(8.0);
    for (i, p) in form.parents.iter().enumerate() {
        let mut text = format!("Parent {}: {}", i + 1, short(p.oid));
        if let Some(s) = &p.summary {
            text.push(' ');
            text.push_str(s);
        }
        if !p.labels.is_empty() {
            text.push_str(&format!(" ({})", p.labels.join(", ")));
        }
        controls::radio_value(ui, &mut form.choice, i, text);
    }
    let pick = buttons::primary(form.op.verb()).enabled(idle).why(Some("Another operation is still running."));
    let out = d.finish(ui, Footer::new().cancel().primary(pick));
    if out.primary {
        let mainline = u32::try_from(form.choice + 1).unwrap_or(1);
        msgs.push(Msg::Selection(SelectionMsg::PickMerge { op: form.op, oid: form.oid, mainline }));
    }
    out.primary || out.dismiss
}
