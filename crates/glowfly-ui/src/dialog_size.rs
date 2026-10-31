//! A dialog's size class (`dialog_frame`): a base width it never goes under and a most it grows
//! to while its title row or subtitle needs the room. Wrapping starts beyond the most, and the
//! window caps it all.

/// The least content width, however small the window.
pub(super) const MIN_WIDTH: f32 = 320.0;

/// How wide a dialog's content is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    /// Confirmations and one-field questions: 400 to 480 px.
    Small,
    /// Most forms: 480 to 560 px.
    #[default]
    Medium,
    /// Long text and lists: 640 to 760 px.
    Wide,
}

impl Size {
    /// The width the content starts at.
    pub fn base(self) -> f32 {
        match self {
            Size::Small => 400.0,
            Size::Medium => 480.0,
            Size::Wide => 640.0,
        }
    }

    /// The most the content grows to for a long title or subtitle.
    pub fn max(self) -> f32 {
        match self {
            Size::Small => 480.0,
            Size::Medium => 560.0,
            Size::Wide => 760.0,
        }
    }
}

/// The content width for `need` (the no-wrap width of the title row or subtitle) in `size`,
/// with `room` free in the window.
pub(super) fn width(size: Size, need: f32, room: f32) -> f32 {
    need.clamp(size.base(), size.max()).min(room.max(MIN_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_is_clamped_to_the_class() {
        for s in [Size::Small, Size::Medium, Size::Wide] {
            assert_eq!(width(s, 10.0, 2000.0), s.base(), "{s:?}");
            assert_eq!(width(s, 5000.0, 2000.0), s.max(), "{s:?}");
            let mid = (s.base() + s.max()) / 2.0;
            assert_eq!(width(s, mid, 2000.0), mid, "{s:?}");
        }
    }

    #[test]
    fn room_caps_it_but_never_under_the_least() {
        assert_eq!(width(Size::Wide, 700.0, 500.0), 500.0);
        assert_eq!(width(Size::Medium, 500.0, 100.0), MIN_WIDTH);
        assert_eq!(width(Size::Small, 0.0, 0.0), MIN_WIDTH);
    }
}
