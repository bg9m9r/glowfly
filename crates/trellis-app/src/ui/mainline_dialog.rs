//! The Choose the mainline parent dialog (`Modal::Mainline`): which parent of a merge commit a
//! cherry-pick or revert counts the merge's changes against.

use super::common::short;
use crate::msg::{Msg, SelectionMsg};
use crate::state::pick::{MainlineForm, PickOp};
use crate::ui::theme::color;
use eframe::egui::{self, Color32, RichText, Stroke};

/// What the dialog explains about the choice.
const EXPLAIN: &str = "A merge commit has more than one parent. The change applied is what the merge brought in compared with the \
                       parent you choose, which is usually parent 1, the branch that was merged into.";

/// Draws the dialog; returns true when it should close (submitted or Cancel). The primary button
/// waits while a job runs (`idle` false).
pub fn show(ui: &mut egui::Ui, form: &mut MainlineForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Choose the mainline parent");
    let what = match form.op {
        PickOp::CherryPick => "Cherry-pick",
        PickOp::Revert => "Revert",
    };
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
        ui.radio_value(&mut form.choice, i, text);
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let (fg, fill) = if idle { (Color32::WHITE, color::primary()) } else { (color::faint(), color::primary_off()) };
        let button = egui::Button::new(RichText::new(form.op.verb()).strong().color(fg))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, color::primary_border()));
        let r = ui.add_enabled(idle, button).on_disabled_hover_text("Another operation is still running.");
        if r.clicked() {
            let mainline = u32::try_from(form.choice + 1).unwrap_or(1);
            msgs.push(Msg::Selection(SelectionMsg::PickMerge { op: form.op, oid: form.oid, mainline }));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
