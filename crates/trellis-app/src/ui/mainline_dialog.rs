//! The Choose the mainline parent dialog (`Modal::Mainline`): which parent of the merge commits a
//! cherry-pick or revert counts their changes against, for one merge or several commits at once.
//! Its side pane explains the choice.

use super::common::short;
use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::widgets::buttons;
use crate::msg::{Msg, SelectionMsg};
use crate::state::pick::{MainlineForm, ParentChoice, PickOp};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;

/// The most merges listed under each parent number; the rest are counted.
const LISTED_MERGES: usize = 3;

/// Draws the dialog; returns true when it should close (submitted or Cancel). The primary button
/// waits while a job runs (`idle` false).
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut MainlineForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, "Choose the mainline parent").guide(dialog_guides::mainline(cx.t)).begin(ui);
    if form.single() {
        one_merge(ui, form);
    } else {
        several(ui, cx.t, form);
    }
    let pick = buttons::primary(form.op.verb()).enabled(idle).why(Some("Another operation is still running."));
    let out = d.finish(ui, Footer::new().cancel().primary(pick));
    if out.primary {
        let mainline = u32::try_from(form.choice + 1).unwrap_or(1);
        msgs.push(Msg::Selection(SelectionMsg::PickWithMainline { op: form.op, oids: form.oids.clone(), mainline }));
    }
    out.primary || out.dismiss
}

/// "abc1234 Summary (main, origin/main)": a parent as the dialog names it.
fn parent_text(p: &ParentChoice) -> String {
    let mut text = short(p.oid);
    if let Some(s) = &p.summary {
        text.push(' ');
        text.push_str(s);
    }
    if !p.labels.is_empty() {
        text.push_str(&format!(" ({})", p.labels.join(", ")));
    }
    text
}

/// One merge commit: a radio for each of its parents.
fn one_merge(ui: &mut egui::Ui, form: &mut MainlineForm) {
    let what = match form.op {
        PickOp::CherryPick => "Cherry-pick",
        PickOp::Revert => "Revert",
    };
    let merge = &form.merges[0];
    ui.label(RichText::new(format!("{what} merge commit {} {}", short(merge.oid), merge.summary)).color(color::text()));
    ui.add_space(8.0);
    for (i, p) in merge.parents.iter().enumerate() {
        controls::radio_value(ui, &mut form.choice, i, format!("Parent {}: {}", i + 1, parent_text(p)));
    }
}

/// Several commits: one radio per parent number every commit has, each listing what that parent
/// is for the merges, and why fewer numbers are offered than the merges have parents.
fn several(ui: &mut egui::Ui, t: Lang, form: &mut MainlineForm) {
    let (n, total) = (form.merges.len(), [("total", &form.oids.len() as &dyn std::fmt::Display)]);
    let lead = match form.op {
        PickOp::CherryPick => t.trn("mainline.several.cherry_pick", n, &total),
        PickOp::Revert => t.trn("mainline.several.revert", n, &total),
    };
    ui.add(egui::Label::new(RichText::new(lead).color(color::text())).wrap());
    ui.add_space(8.0);
    let weak = |ui: &mut egui::Ui, text: String| {
        ui.add(egui::Label::new(RichText::new(text).color(color::weak())).truncate());
    };
    for n in 0..form.choices() {
        controls::radio_value(ui, &mut form.choice, n, t.trf("mainline.parent", &[("n", &(n + 1))]));
        ui.indent(("mainline-parent", n), |ui| {
            for m in form.merges.iter().take(LISTED_MERGES) {
                if let Some(p) = m.parents.get(n) {
                    weak(ui, format!("{}: {}", short(m.oid), parent_text(p)));
                }
            }
            if form.merges.len() > LISTED_MERGES {
                weak(ui, t.trn("mainline.more_merges", form.merges.len() - LISTED_MERGES, &[]));
            }
        });
    }
    if form.choices() < form.most_parents() {
        let note = match form.plain {
            Some(o) => t.trf("mainline.limited.plain", &[("sha", &short(o))]),
            None => {
                let fewest = form.merges.iter().min_by_key(|m| m.parents.len()).map(|m| (m.oid, m.parents.len()));
                let (oid, count) = fewest.unwrap_or((form.oids[0], 1));
                t.trf("mainline.limited.parents", &[("sha", &short(oid)), ("count", &count)])
            }
        };
        ui.add_space(8.0);
        ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
    }
}
