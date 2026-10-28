//! The spacing grid: every gap, padding and inset is a multiple of 4 px (an 8 px grid with its
//! half step), and side panels keep their content 16–20 px from their edges.

use eframe::egui::{Vec2, vec2};

/// The grid step.
pub const GRID: f32 = 8.0;
/// egui's gap between widgets: a full step between widgets in a row, the half step between
/// stacked ones (so long context menus still fit a small window).
pub const ITEM_SPACING: Vec2 = vec2(8.0, 4.0);
/// The padding inside egui's buttons.
pub const BUTTON_PADDING: Vec2 = vec2(8.0, 4.0);
/// The inner margin of side panels (the details panel, the issue and pull request panels), as
/// egui's `Margin` takes it.
pub const PANEL_PAD: i8 = 16;
/// `PANEL_PAD` in points.
pub const PANEL_PAD_F: f32 = PANEL_PAD as f32;

/// Menu rows and frames, applied by `widgets::menu::style` in every context menu, dropdown,
/// menu-bar menu and split-button menu.
pub mod menu {
    /// One density's row metrics.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Metrics {
        /// A row's height: the hover pill's.
        pub row_h: f32,
        /// The space between two rows' pills.
        pub row_gap: f32,
        /// The text's inset from the pill's edge.
        pub pad_x: f32,
        /// The narrowest and widest a menu grows.
        pub min_w: f32,
        pub max_w: f32,
    }

    /// The default: 28 px rows on a 30 px pitch.
    pub const COMFORTABLE: Metrics = Metrics { row_h: 28.0, row_gap: 2.0, pad_x: 12.0, min_w: 200.0, max_w: 380.0 };
    /// The opt-in dense variant: 22 px rows on a 24 px pitch (no setting picks it yet).
    #[allow(dead_code)]
    pub const COMPACT: Metrics = Metrics { row_h: 22.0, row_gap: 2.0, pad_x: 8.0, min_w: 180.0, max_w: 340.0 };
    /// The menu frame's inner margin, so pills sit this far from its edge (egui's `Margin`).
    pub const FRAME_PAD: i8 = 4;
    /// The space between items in a row (a label and its shortcut).
    pub const GAP_X: f32 = 8.0;
    /// A separator's height, its 1 px line in the middle: with the rows' gaps, 4 px between
    /// the line and the pills on either side.
    pub const SEPARATOR_H: f32 = 4.0;
}
