//! The pull request panel's details: status lines and reviewers.

use super::super::theme::color;
use super::{note, tone_color};
use crate::state::pull_request::{PrView, review_summary, reviewer_tone};
use crate::ui::theme::type_scale;
use eframe::egui::{RichText, Ui};
use trellis_core::forge::PrState;
use trellis_core::text::printable;

pub(super) fn show(ui: &mut Ui, v: &PrView) {
    status(ui, v);
    reviewers(ui, v);
}

/// Draft, merged or closed, the service's merge status, and whether the last refresh still
/// listed it (only while it may still be open).
fn status(ui: &mut Ui, v: &PrView) {
    let pr = &v.pr;
    let mut any = false;
    if pr.draft {
        note(ui, "Draft: still in progress, not ready to merge.", color::warn());
        any = true;
    }
    match v.state() {
        Some(PrState::Merged) => note(ui, format!("Merged into {}", printable(&pr.target)), color::added()),
        Some(PrState::Closed) => note(ui, "Closed without merging", color::weak()),
        _ => {}
    }
    any |= matches!(v.state(), Some(PrState::Merged | PrState::Closed));
    if let Some(ms) = &pr.merge_status {
        note(ui, format!("Merge status: {}", printable(ms)), color::text());
        any = true;
    }
    if !v.listed && matches!(v.state(), None | Some(PrState::Open)) {
        note(ui, format!("No longer in the list of open {}s.", v.noun), color::weak());
        any = true;
    }
    if any {
        ui.add_space(8.0);
    }
}

/// The approval summary, then each reviewer with their state.
fn reviewers(ui: &mut Ui, v: &PrView) {
    ui.horizontal(|ui| {
        ui.label(type_scale::section_text(ui, "Reviewers"));
        ui.label(RichText::new(review_summary(&v.pr)).color(color::weak()));
    });
    for r in &v.pr.reviewers {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(printable(&r.name)).color(color::text()));
            ui.label(RichText::new(printable(&r.state)).color(tone_color(reviewer_tone(&r.state))));
            if r.required {
                ui.label(RichText::new(" (required)").color(color::weak()));
            }
        });
    }
}
