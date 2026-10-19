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
