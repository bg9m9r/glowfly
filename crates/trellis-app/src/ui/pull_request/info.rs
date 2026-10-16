//! The pull request panel's details: status lines, reviewers and the description.

use super::super::theme::color;
use crate::state::pull_request::{PrView, Tone, description_text, review_summary, reviewer_tone};
use eframe::egui::{self, Color32, RichText, Ui};
use trellis_core::text::printable;

pub(super) fn show(ui: &mut Ui, v: &PrView) {
    status(ui, v);
    reviewers(ui, v);
    ui.add_space(8.0);
    ui.label(RichText::new("Description").strong().color(color::text()));
    let d = description_text(v.pr.description.trim());
    if d.is_empty() {
        ui.label(RichText::new("No description.").color(color::weak()));
    } else {
        ui.add(egui::Label::new(RichText::new(d).color(color::text())).wrap());
    }
    ui.add_space(4.0);
}

/// Draft, the service's merge status, and whether the last refresh still listed it.
fn status(ui: &mut Ui, v: &PrView) {
    let pr = &v.pr;
    let mut any = false;
    if pr.draft {
        ui.add(egui::Label::new(RichText::new("Draft: still in progress, not ready to merge.").color(color::amber())).wrap());
        any = true;
    }
    if let Some(ms) = &pr.merge_status {
        ui.add(egui::Label::new(RichText::new(format!("Merge status: {}", printable(ms))).color(color::text())).wrap());
        any = true;
    }
    if !v.listed {
        let text = format!("No longer in the list of open {}s.", v.noun);
        ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
        any = true;
    }
    if any {
        ui.add_space(8.0);
    }
}

/// The approval summary, then each reviewer with their state.
fn reviewers(ui: &mut Ui, v: &PrView) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Reviewers").strong().color(color::text()));
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

fn tone_color(t: Tone) -> Color32 {
    match t {
        Tone::Good => color::green(),
        Tone::Bad => color::red(),
        Tone::Neutral => color::weak(),
    }
}
