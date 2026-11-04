//! The issue panel's side: the details (state or status as a chip, type, assignees, labels as
//! chips, milestone or fix version, epic, who opened it and when), the local branches named
//! after it with a Checkout each (Open worktree for one another worktree has), and the actions
//! (Create branch…, the Git Flow starts, Copy).

use super::super::common::state_color;
use super::super::pull_request::{heading, note};
use super::super::theme::color;
use crate::theme::type_scale;
use crate::widgets::buttons;
use crate::widgets::chip::{Chip, ChipSize, Look};
use eframe::egui::{self, Align, Color32, Layout, RichText, Ui, vec2};
use glowfly_core::forge;
use glowfly_core::gitflow::FlowKind;
use glowfly_core::text::printable;
use glowfly_model::gitflow::GitFlowMsg;
use glowfly_model::msg::{BranchMsg, Msg};
use glowfly_model::state::AppState;
use glowfly_model::state::gitflow::start_blocked;
use glowfly_model::state::issue::{IssueView, linked_branches, meta_rows, state_label};
use glowfly_model::state::issue_words::labels_label;
use glowfly_model::state::worktree_branches::Elsewhere;

/// The width of the labels left of the values.
const LABEL_W: f32 = 92.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(v), Some(fs)) = (&st.session.issue.view, &st.session.forge) else { return };
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
                    // One another worktree has: Open worktree (or Prune and check out a missing one).
                    let t = st.settings.lang();
                    let elsewhere = glowfly_model::state::worktree_branches::checkout_elsewhere(t, snap, &b.name);
                    let blocked = if b.is_head {
                        Some("Checked out")
                    } else if !idle && elsewhere.is_none() {
                        Some("Another operation is running")
                    } else {
                        None
                    };
                    let label = elsewhere.as_ref().map_or("Checkout", |e| e.button(t));
                    let r = ui.add(buttons::secondary(label).enabled(blocked.is_none()));
                    let r = match (blocked, &elsewhere) {
                        (Some(why), _) => r.on_hover_text(why),
                        (None, Some(e)) => r.on_hover_text(e.tip()),
                        (None, None) => r,
                    };
                    if r.clicked() {
                        let checkout = BranchMsg::Checkout(b.name.clone());
                        msgs.push(Msg::Branch(Elsewhere::msg(elsewhere.as_ref(), checkout)));
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
    for (label, value) in meta_rows(v, &st.settings, st.cues.utc_offset) {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(LABEL_W, 20.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_width(LABEL_W);
                ui.label(RichText::new(label).color(color::weak()));
            });
            if label == state_row && !value.is_empty() {
                chip(ui, ChipSize::Badge, &value, state_color(&value));
            } else if label == labels_row {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for l in &v.issue.facets.labels {
                        chip(ui, ChipSize::Chip, l, color::text());
                    }
                });
            } else {
                let r = ui.add(egui::Label::new(RichText::new(printable(&value)).color(color::text())).wrap());
                if let Some(c) = v.created().filter(|_| label == "Created") {
                    r.on_hover_text(st.settings.date_tooltip(c, st.cues.now, st.cues.utc_offset));
                }
            }
        });
    }
}

/// A chip of `size` with `text` in `fg` on the raised fill: the state's badge, or a label.
pub(crate) fn chip(ui: &mut Ui, size: ChipSize, text: &str, fg: Color32) {
    Chip::new(size, printable(text)).look(Look::new(color::raised(), fg)).show(ui);
}
