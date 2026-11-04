//! The mark in a tab's state slot (design pass D15, laid out by `tab_paint.rs`): a running job's
//! dots in the accent, a failure's warning triangle and an operation in progress as an amber
//! ring (both `warn`), uncommitted changes as a `weak` dot, or how far the branch is behind as
//! "↓3" (`faint`, 11 px). The tab chrome itself never draws status colours
//! (`theme/vivid_tests.rs`); these marks are the one place a tab shows its repository's state,
//! as the design pass decided, so they live apart from the chrome.

use super::list_row::{ARROW_W, paint_arrow};
use super::tab_paint::SLOT;
use super::theme::{color, type_scale};
use super::widgets::{Icon, dots, paint_icon_sized};
use crate::state::tabs::glance::Mark;
use eframe::egui::{self, Rect, pos2};

/// A behind count's text size.
const BEHIND_TEXT: f32 = type_scale::CAPTION;
/// The changes mark's dot.
const DOT_R: f32 = 3.0;
/// The in-progress mark's ring: 8 px across with its 1.5 px line.
const RING_W: f32 = 1.5;
const RING_R: f32 = 4.0 - RING_W / 2.0;

/// Paints `mark` in a tab's state slot `slot`.
pub(super) fn paint(p: &egui::Painter, slot: Rect, mark: Mark, pulse: Option<f64>) {
    let c = slot.center();
    match mark {
        Mark::Running => dots::paint(p, c, color::accent(), pulse),
        Mark::Failed => paint_icon_sized(p, c, Icon::Warning, color::warn(), SLOT),
        // A ring, so it differs from the changes dot by shape, not only by hue.
        Mark::InProgress => {
            p.circle_stroke(c, RING_R, egui::Stroke::new(RING_W, color::warn()));
        }
        Mark::Changed => {
            p.circle_filled(c, DOT_R, color::weak());
        }
        Mark::Behind(n) => {
            // The count right-aligned in the slot, its painted arrow before it (the bundled
            // fonts have no arrows); a long count reaches left into the gap before the slot.
            let galley = p.layout_no_wrap(n.to_string(), type_scale::font(BEHIND_TEXT), color::faint());
            let x = slot.right() - galley.size().x;
            p.galley(pos2(x, c.y - galley.size().y / 2.0), galley, color::faint());
            paint_arrow(p, pos2(x - ARROW_W / 2.0 - 0.5, c.y), false, color::faint());
        }
    }
}
