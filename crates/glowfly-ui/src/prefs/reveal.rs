//! The row the Preferences search opened: scrolled into view once, then tinted with the
//! selected fill, fading out over `TINT_SECS` (solid for that long, then off, while motion is
//! reduced). The target lives in egui's memory with its page: the search sets it when Enter
//! opens a match, `sync` drops it once its navigation didn't happen (refused, or answered
//! "stay" at "Save your changes?"), and `row` finds it by its label on that page only.

use crate::theme::color;
use eframe::egui::{self, Id, Rect, Ui, pos2};
use glowfly_model::state::PrefPage;

/// How long the tint shows.
const TINT_SECS: f64 = 1.2;

/// The row to reveal: its label and page, the page the search was on, and when the reveal
/// started (None until it is drawn).
#[derive(Clone, Debug, PartialEq)]
struct Target {
    label: String,
    page: PrefPage,
    from: PrefPage,
    since: Option<f64>,
}

fn key() -> Id {
    Id::new("prefs-reveal")
}

/// The page being drawn this frame.
fn drawn_key() -> Id {
    Id::new("prefs-reveal-drawn")
}

/// Reveals the row labelled `label` on `page`, opened from `from`.
pub fn set(ctx: &egui::Context, label: &str, page: PrefPage, from: PrefPage) {
    ctx.data_mut(|d| d.insert_temp(key(), Target { label: label.to_owned(), page, from, since: None }));
}

/// Before the frame's drawing: notes the page drawn (`drawn`), and keeps the target only on its
/// page, or on the page it was opened from while `asking` (the "Save your changes?" question
/// holds the navigation). Anywhere else its navigation didn't happen: it is dropped.
pub fn sync(ctx: &egui::Context, drawn: PrefPage, asking: bool) {
    ctx.data_mut(|d| {
        d.insert_temp(drawn_key(), drawn);
        let keep = d.get_temp::<Target>(key()).is_none_or(|t| t.page == drawn || (t.from == drawn && asking));
        if !keep {
            d.remove::<Target>(key());
        }
    });
}

/// The row labelled `label` on the page drawn is the one to reveal: the painter slot for its
/// tint.
pub fn begin(ui: &mut Ui, label: &str) -> Option<egui::layers::ShapeIdx> {
    let (target, drawn) = ui.ctx().data(|d| (d.get_temp::<Target>(key()), d.get_temp::<PrefPage>(drawn_key())));
    let target = target?;
    (!label.is_empty() && target.label == label && Some(target.page) == drawn).then(|| ui.painter().add(egui::Shape::Noop))
}

/// Tints the revealed row (drawn from `top` down to the cursor), scrolling to it the first time.
pub fn end(ui: &mut Ui, slot: Option<egui::layers::ShapeIdx>, top: f32) {
    let Some(slot) = slot else { return };
    let left = ui.min_rect().left();
    let rect = Rect::from_min_max(pos2(left, top - 6.0), pos2(left + super::CONTENT_W, ui.cursor().top() + 6.0));
    let now = ui.input(|i| i.time);
    let Some(mut target) = ui.ctx().data(|d| d.get_temp::<Target>(key())) else { return };
    let since = *target.since.get_or_insert_with(|| {
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
        now
    });
    ui.ctx().data_mut(|d| d.insert_temp(key(), target));
    // Reduced motion sets no animation time: the tint stays solid, then goes.
    let alpha = strength(now - since, ui.style().animation_time == 0.0);
    if alpha <= 0.0 {
        ui.ctx().data_mut(|d| d.remove::<Target>(key()));
        return;
    }
    let fill = color::selected().gamma_multiply(alpha);
    ui.painter().set(slot, egui::Shape::rect_filled(rect, 4.0, fill));
    ui.ctx().request_repaint();
}

/// The tint's strength `elapsed` seconds in: full, fading to none at `TINT_SECS` (or full until
/// then when `still`).
fn strength(elapsed: f64, still: bool) -> f32 {
    let left = TINT_SECS - elapsed;
    if left <= 0.0 {
        0.0
    } else if still {
        1.0
    } else {
        (left / TINT_SECS) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(ctx: &egui::Context) -> Option<PrefPage> {
        ctx.data(|d| d.get_temp::<Target>(key())).map(|t| t.page)
    }

    #[test]
    fn a_reveal_waits_for_its_page_and_goes_when_the_navigation_does_not_happen() {
        let ctx = egui::Context::default();
        set(&ctx, "Interface size", PrefPage::Ui, PrefPage::Accessibility);
        sync(&ctx, PrefPage::Accessibility, true);
        assert_eq!(target(&ctx), Some(PrefPage::Ui), "kept while Save your changes? holds the navigation");
        sync(&ctx, PrefPage::Ui, false);
        assert_eq!(target(&ctx), Some(PrefPage::Ui), "kept on its page");
        set(&ctx, "Interface size", PrefPage::Ui, PrefPage::Accessibility);
        sync(&ctx, PrefPage::Accessibility, false);
        assert_eq!(target(&ctx), None, "staying on the page drops it");
        set(&ctx, "Interface size", PrefPage::Ui, PrefPage::Accessibility);
        sync(&ctx, PrefPage::General, false);
        assert_eq!(target(&ctx), None, "so does going elsewhere");
    }

    #[test]
    fn the_tint_fades_over_its_time_or_holds_while_still() {
        assert_eq!(strength(0.0, false), 1.0);
        assert!((strength(0.6, false) - 0.5).abs() < 1e-6);
        assert_eq!(strength(1.2, false), 0.0);
        assert_eq!(strength(0.9, true), 1.0);
        assert_eq!(strength(1.3, true), 0.0);
    }
}
