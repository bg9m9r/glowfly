//! The issue panel's side: the details (state or status as a chip, type, assignees, labels as
//! chips, milestone or fix version, epic, who opened it and when), the local branches named
//! after it with a Checkout each, and the actions (Create branch…, the Git Flow starts, Copy).

use super::super::common::state_color;
use super::super::pull_request::{heading, note};
use super::super::theme::color;
use super::super::widgets;
use crate::gitflow::GitFlowMsg;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::state::gitflow::start_blocked;
use crate::state::issue::{IssueView, linked_branches, meta_rows, state_label};
use crate::state::issue_words::labels_label;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Ui, vec2};
use trellis_core::forge;
use trellis_core::gitflow::FlowKind;
use trellis_core::text::printable;

/// The width of the labels left of the values.
const LABEL_W: f32 = 92.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(v), Some(fs)) = (&st.issue, &st.forge) else { return };
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.label(type_scale::section_text(ui, "Details"));
    details(ui, v, st);
    heading(ui, "Branches");
    let idle = st.job.is_none();
    match st.snap.as_ref() {
        Some(snap) => {
            let linked = linked_branches(snap, fs, &v.key);
            if linked.is_empty() {
                ui.label(RichText::new("No branches for this issue yet.").color(color::faint()));
            }
            for b in linked {
                ui.horizontal(|ui| {
                    let elsewhere = crate::state::worktree_branches::checkout_blocked(snap, &b.name);
                    let blocked = if b.is_head {
                        Some("Checked out")
                    } else if let Some(why) = elsewhere.as_deref() {
                        Some(why)
                    } else if !idle {
                        Some("Another operation is running")
                    } else {
                        None
                    };
                    let r = ui.add(buttons::secondary("Checkout").enabled(blocked.is_none()));
                    let r = match blocked {
                        Some(why) => r.on_hover_text(why),
                        None => r,
                    };
                    if r.clicked() {
                        msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.clone())));
                    }
                    ui.add(egui::Label::new(RichText::new(printable(&b.name)).color(color::text())).truncate());
                });
            }
        }
        None => note(ui, "No branches for this issue yet.", color::faint()),
    }
    heading(ui, "Actions");
    if ui.add(buttons::secondary("Create branch…")).clicked() {
        msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: forge::branch_name_for(&v.issue) }));
    }
    if let Some(snap) = st.snap.as_ref().filter(|s| s.gitflow.is_initialized()) {
        for k in FlowKind::ALL {
            let blocked = start_blocked(snap, k);
            let r = ui.add(buttons::secondary(format!("Start Git Flow {}…", k.noun())).enabled(blocked.is_none()));
            let r = match &blocked {
                Some(why) => r.on_hover_text(why),
                None => r,
            };
            if r.clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::StartFromIssue { kind: k, name: forge::branch_name_for(&v.issue) }));
            }
        }
    }
    let (label, text) = fs.copy_issue_item(&v.issue);
    if ui.add(buttons::secondary(label)).clicked() {
        msgs.push(Msg::Copy(text));
    }
}

/// One row per detail; the state and the labels as chips.
fn details(ui: &mut Ui, v: &IssueView, st: &AppState) {
    let (state_row, labels_row) = (state_label(v.provider()), labels_label(v.provider()));
    for (label, value) in meta_rows(v, &st.settings, st.view.utc_offset) {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(LABEL_W, 20.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_width(LABEL_W);
                ui.label(RichText::new(label).color(color::weak()));
            });
            if label == state_row && !value.is_empty() {
                chip(ui, &value, state_color(&value));
            } else if label == labels_row {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for l in &v.issue.facets.labels {
                        chip(ui, l, color::text());
                    }
                });
            } else {
                let r = ui.add(egui::Label::new(RichText::new(printable(&value)).color(color::text())).wrap());
                if let Some(c) = v.created().filter(|_| label == "Created") {
                    r.on_hover_text(st.settings.date_tooltip(c, st.view.now, st.view.utc_offset));
                }
            }
        });
    }
}

/// A rounded chip with `text` in `fg`.
fn chip(ui: &mut Ui, text: &str, fg: Color32) {
    let text = printable(text);
    let w = ui.painter().layout_no_wrap(text.clone(), type_scale::font(type_scale::CAPTION), fg).size().x + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    widgets::chip(ui.painter(), rect.left(), rect.center().y, &text, color::raised(), fg);
}
