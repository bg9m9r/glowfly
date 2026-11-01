//! Control and chip sizes: every control's height, side padding, text step, icon size, icon gap,
//! corners and least hit area (`ControlSize`, `Metrics`), the two chip and badge sizes
//! (`ChipSize`, `ChipMetrics`), where text sits in a control (`text_pos`) and which controls a
//! row of a given height can host (`fits`).
//!
//! Sizes only measure: they never paint a state. The state layers are painted by
//! `widgets::button_look::paint_target`, which takes a size but never chooses one.

use super::tokens::radius;
use super::type_scale::{BODY, CAPTION, SMALL};
use crate::ui::widgets::icon::{SIZE_MD, SIZE_SM};
use eframe::egui::{Galley, Pos2, Rect, pos2};

/// A control's size. Compact is for quiet controls only (`allows_frame`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 22 px: quiet controls in 28 px rows (menus, palette rows).
    Compact,
    /// 24 px: controls inline with a field, a row or a line of chips.
    Small,
    /// 28 px: the standard control.
    #[default]
    Regular,
    /// 36 px: dialog fields, the commit button.
    Large,
}

/// A control size's measures, in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The height; an icon-only control is a square of this side.
    pub h: f32,
    /// Room beside the content of a bordered or filled control (0 for Compact, which can't be
    /// either).
    pub pad_x: f32,
    /// Room beside the content of a quiet control: its framed sibling's, so hover shapes match
    /// in a row; only Compact is denser.
    pub quiet_pad_x: f32,
    /// The text step.
    pub text: f32,
    /// The icon size, alone or before the text.
    pub icon: f32,
    /// Between an icon and the text: 4 beside a 14 px icon, 6 beside a 16 px one (8 in Large).
    pub icon_gap: f32,
    /// The corners.
    pub radius: u8,
    /// The least hit area's side.
    pub hit: f32,
}

impl ControlSize {
    /// The size's measures.
    pub const fn metrics(self) -> Metrics {
        match self {
            Self::Compact => {
                Metrics { h: 22.0, pad_x: 0.0, quiet_pad_x: 8.0, text: SMALL, icon: SIZE_SM, icon_gap: 4.0, radius: radius::MD, hit: 28.0 }
            }
            Self::Small => Metrics {
                h: 24.0,
                pad_x: 10.0,
                quiet_pad_x: 10.0,
                text: SMALL,
                icon: SIZE_MD,
                icon_gap: 6.0,
                radius: radius::MD,
                hit: 28.0,
            },
            Self::Regular => {
                Metrics { h: 28.0, pad_x: 12.0, quiet_pad_x: 12.0, text: BODY, icon: SIZE_MD, icon_gap: 6.0, radius: radius::MD, hit: 28.0 }
            }
            Self::Large => {
                Metrics { h: 36.0, pad_x: 16.0, quiet_pad_x: 16.0, text: BODY, icon: SIZE_MD, icon_gap: 8.0, radius: radius::LG, hit: 36.0 }
            }
        }
    }

    /// Whether a bordered or filled control may take this size: every size but Compact.
    pub const fn allows_frame(self) -> bool {
        !matches!(self, Self::Compact)
    }

    /// The least room above and below the text inside the control: 4 px each side for a
    /// bordered or filled one, 3 px for a quiet one (the inset invariant).
    pub const fn inset(bordered: bool) -> f32 {
        if bordered { 8.0 } else { 6.0 }
    }
}

/// A chip's or badge's size.
// Painted by `widgets/chip.rs` from sizing slice S3; until then only its tests read it.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChipSize {
    /// 18 px pills: counts and status letters, the tab chip, pull request and build counts.
    Badge,
    /// 22 px: refs and tags, labels, identity, SHA and parent chips, the signature badge, SSH
    /// pills, picker chips.
    Chip,
}

/// A chip size's measures, in px.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChipMetrics {
    pub h: f32,
    pub pad_x: f32,
    pub text: f32,
    /// A leading glyph's size, and its gap to the text.
    pub glyph: f32,
    pub gap: f32,
    /// The corners: a pill for a badge (`radius::pill(h)`), `radius::SM` for a chip.
    pub radius: u8,
    /// The least width (a badge is at least a circle).
    pub min_w: f32,
}

#[allow(dead_code)]
impl ChipSize {
    /// The size's measures.
    pub const fn metrics(self) -> ChipMetrics {
        match self {
            Self::Badge => ChipMetrics { h: 18.0, pad_x: 6.0, text: CAPTION, glyph: 10.0, gap: 4.0, radius: 9, min_w: 18.0 },
            Self::Chip => ChipMetrics { h: 22.0, pad_x: 8.0, text: SMALL, glyph: 12.0, gap: 4.0, radius: radius::SM, min_w: 0.0 },
        }
    }
}

/// Where `galley` goes to sit centred in `rect` from its left edge: its top on a whole point
/// above the centre line (egui then snaps it to physical pixels), so text never sits low.
pub fn text_pos(rect: Rect, galley: &Galley) -> Pos2 {
    pos2(rect.left(), centred_top(rect, galley.size().y))
}

/// The top of a block `h` tall centred in `rect`, rounded as `text_pos` rounds: for a stack of
/// galleys (a caption over a name).
pub fn centred_top(rect: Rect, h: f32) -> f32 {
    (rect.center().y - h / 2.0).floor()
}

/// Whether a row `row_h` tall can host a control of `size`: 4 px clear above and below a
/// bordered or filled one, 3 px for a quiet one.
pub fn fits(row_h: f32, size: ControlSize, bordered: bool) -> bool {
    row_h >= size.metrics().h + ControlSize::inset(bordered)
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod tests;
