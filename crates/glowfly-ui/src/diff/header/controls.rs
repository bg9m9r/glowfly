//! The header's controls: the shared ghost controls (`widgets::ghost`), at the Regular size.

use crate::ui::theme::sizes::ControlSize;
pub(super) use crate::ui::widgets::ghost::{DIVIDER_W, ICON, SPACE, divider, font, ghost, ghost_width, ghost_with, icon_tip, text_w};

/// A ghost control's height, and an icon-only one's width.
pub(super) const HEIGHT: f32 = ControlSize::Regular.metrics().h;
