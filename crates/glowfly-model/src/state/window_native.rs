//! The pure parts of the drawn Windows frame's glue with the OS (`platform::native_window`):
//! the Maximize button as Windows tracks it for Snap Layouts (`CaptionButton`), where the
//! caption parts are and what Windows' hit test answers over them (`CaptionGeometry::hit`),
//! the system menu's item states, and whether the title
//! bar, toolbar and sidebar sit on the Mica backdrop (`Backdrop`). Tested on every OS; only the
//! Windows glue drives most of it.

// Only the Windows glue calls most of these outside tests.
#![cfg_attr(not(windows), allow(dead_code))]

/// A rectangle in client pixels; `right` and `bottom` are outside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PxRect {
    pub fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        !self.is_empty() && (self.left..self.right).contains(&x) && (self.top..self.bottom).contains(&y)
    }
}

/// Where the drawn caption parts are, in client pixels (`ui::window_frame::caption_geometry`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptionGeometry {
    /// The Maximize (or Restore) button.
    pub max_button: PxRect,
    /// How many pixels at the button's top resize the window instead, like the edge strip
    /// beside it: 0 while maximized or full screen.
    pub top_edge: i32,
    /// Where Alt+Space opens the system menu: just below the title bar's menu button.
    pub menu_at: (i32, i32),
}

/// What Windows' hit test (`WM_NCHITTEST`) answers over a drawn caption part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionHit {
    /// The top edge: Windows resizes the window from it (`HTTOP`).
    Top,
    /// The Maximize button: Windows 11 shows Snap Layouts over it (`HTMAXBUTTON`).
    MaxButton,
}

impl CaptionGeometry {
    /// The answer at client point (`x`, `y`); None leaves the point to egui (the client area).
    pub fn hit(self, x: i32, y: i32) -> Option<CaptionHit> {
        if !self.max_button.contains(x, y) {
            return None;
        }
        Some(if y < self.max_button.top + self.top_edge { CaptionHit::Top } else { CaptionHit::MaxButton })
    }
}

/// The drawn Maximize button while Windows tracks it: hovered, and pressed on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaptionButton {
    pub hovered: bool,
    pub pressed: bool,
}

/// What the OS reported about the pointer and the Maximize button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionEvent {
    /// The pointer moved, over the button or elsewhere on the frame.
    Move { over: bool },
    /// The left button went down.
    Down { over: bool },
    /// The left button came up.
    Up { over: bool },
    /// The pointer left the window's frame.
    Leave,
}

impl CaptionButton {
    /// The button after `e`, and whether `e` finished a click: pressed and released over it.
    pub fn step(self, e: CaptionEvent) -> (CaptionButton, bool) {
        match e {
            CaptionEvent::Move { over } => (CaptionButton { hovered: over, ..self }, false),
            CaptionEvent::Down { over: true } => (CaptionButton { hovered: true, pressed: true }, false),
            CaptionEvent::Down { over: false } => (self, false),
            CaptionEvent::Up { over } => (CaptionButton { hovered: over, pressed: false }, self.pressed && over),
            CaptionEvent::Leave => (CaptionButton::default(), false),
        }
    }

    /// Drawn pressed: pressed on it and the pointer is still over it.
    pub fn drawn_pressed(self) -> bool {
        self.pressed && self.hovered
    }
}

/// The system menu's items (their wording comes from Windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysItem {
    Restore,
    Move,
    Size,
    Minimize,
    Maximize,
    Close,
}

/// Each system menu item and whether it is enabled: a maximized window can be restored but not
/// moved, sized or maximized; a restored one can't be restored.
pub fn system_menu_items(maximized: bool) -> [(SysItem, bool); 6] {
    [
        (SysItem::Restore, maximized),
        (SysItem::Move, !maximized),
        (SysItem::Size, !maximized),
        (SysItem::Minimize, true),
        (SysItem::Maximize, !maximized),
        (SysItem::Close, true),
    ]
}

/// What the title bar, toolbar and sidebar are painted on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backdrop {
    /// Their usual opaque colours.
    #[default]
    Solid,
    /// Their colours at these alphas over the Mica backdrop: `chrome` for the title bar and
    /// toolbar, `side` for the sidebar.
    Mica { chrome: u8, side: u8 },
}

/// What the OS allows, read by the Windows glue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackdropSupport {
    /// Windows accepted the Mica backdrop for the window.
    pub supported: bool,
    /// Settings > Personalization > Colors > Transparency effects is on.
    pub transparency: bool,
    /// Windows' high contrast is on.
    pub high_contrast: bool,
}

/// The tint alphas that keep the palette's text readable over Mica (`ui::backdrop::mica_tint`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MicaTint {
    pub chrome: u8,
    pub side: u8,
}

/// Mica only when the OS supports and allows it and the palette has a readable tint.
pub fn backdrop(s: BackdropSupport, tint: Option<MicaTint>) -> Backdrop {
    match tint {
        Some(t) if s.supported && s.transparency && !s.high_contrast => Backdrop::Mica { chrome: t.chrome, side: t.side },
        _ => Backdrop::Solid,
    }
}

/// The first Windows build with the documented system backdrop attribute (Windows 11 22H2).
const MICA_MIN_BUILD: u32 = 22621;

/// Whether Windows build `build` can show Mica behind the window.
pub fn mica_build(build: Option<u32>) -> bool {
    build.is_some_and(|b| b >= MICA_MIN_BUILD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use CaptionEvent::{Down, Leave, Move, Up};

    /// Runs `events` from a fresh button; returns how many clicks they made and the last state.
    fn run(events: &[CaptionEvent]) -> (usize, CaptionButton) {
        let mut b = CaptionButton::default();
        let mut clicks = 0;
        for e in events {
            let (next, clicked) = b.step(*e);
            b = next;
            clicks += usize::from(clicked);
        }
        (clicks, b)
    }

    #[test]
    fn caption_button_click_needs_press_and_release_over_it() {
        assert_eq!(run(&[Move { over: true }, Down { over: true }, Up { over: true }]).0, 1);
        assert_eq!(run(&[Down { over: true }, Move { over: false }, Up { over: false }]).0, 0, "released off it");
        assert_eq!(run(&[Down { over: false }, Up { over: true }]).0, 0, "pressed elsewhere");
        assert_eq!(run(&[Down { over: true }, Leave, Up { over: true }]).0, 0, "leaving cancels");
        let double = [Down { over: true }, Up { over: true }, Down { over: true }, Up { over: true }];
        assert_eq!(run(&double).0, 2, "a double-click toggles twice");
        let (_, b) = run(&[Down { over: true }, Move { over: false }]);
        assert!(b.pressed && !b.drawn_pressed(), "not drawn pressed while the pointer is off it");
        let (_, b) = run(&[Down { over: true }, Move { over: false }, Move { over: true }]);
        assert!(b.drawn_pressed(), "pressed again on coming back");
        assert_eq!(run(&[Move { over: true }]).1, CaptionButton { hovered: true, pressed: false });
    }

    #[test]
    fn px_rect_contains_is_half_open() {
        let r = PxRect { left: 10, top: 0, right: 20, bottom: 5 };
        assert!(r.contains(10, 0) && r.contains(19, 4));
        assert!(!r.contains(20, 0) && !r.contains(10, 5) && !r.contains(9, 0) && !r.contains(10, -1));
        let empty = PxRect::default();
        assert!(empty.is_empty() && !empty.contains(0, 0));
        assert!(PxRect { left: 5, top: 0, right: 5, bottom: 9 }.is_empty());
    }

    #[test]
    fn the_top_strip_over_maximize_resizes_and_the_rest_snaps() {
        let max_button = PxRect { left: 1348, top: 0, right: 1394, bottom: 36 };
        let g = CaptionGeometry { max_button, top_edge: 5, menu_at: (8, 44) };
        for (x, y, hit) in [
            (1348, 0, Some(CaptionHit::Top)),
            (1393, 4, Some(CaptionHit::Top)),
            (1370, 5, Some(CaptionHit::MaxButton)),
            (1393, 35, Some(CaptionHit::MaxButton)),
            (1347, 2, None),
            (1394, 2, None),
            (1370, 36, None),
            (700, 2, None),
        ] {
            assert_eq!(g.hit(x, y), hit, "({x}, {y})");
        }
        // Maximized: no resize strip, the whole button snaps.
        let maximized = CaptionGeometry { top_edge: 0, ..g };
        assert_eq!(maximized.hit(1370, 0), Some(CaptionHit::MaxButton));
        // Full screen: no button for Windows at all.
        let full = CaptionGeometry { max_button: PxRect::default(), top_edge: 0, ..g };
        assert_eq!(full.hit(0, 0), None);
        // A strip as tall as the button leaves no Snap Layouts, never a hit outside it.
        let all_edge = CaptionGeometry { top_edge: 50, ..g };
        assert_eq!(all_edge.hit(1370, 35), Some(CaptionHit::Top));
        assert_eq!(all_edge.hit(1370, 40), None);
    }

    #[test]
    fn system_menu_items_follow_the_window() {
        let on = |maximized| system_menu_items(maximized).iter().filter(|(_, on)| *on).map(|(i, _)| *i).collect::<Vec<_>>();
        assert_eq!(on(true), [SysItem::Restore, SysItem::Minimize, SysItem::Close]);
        assert_eq!(on(false), [SysItem::Move, SysItem::Size, SysItem::Minimize, SysItem::Maximize, SysItem::Close]);
    }

    #[test]
    fn backdrop_truth_table() {
        let tint = Some(MicaTint { chrome: 166, side: 191 });
        let all = BackdropSupport { supported: true, transparency: true, high_contrast: false };
        assert_eq!(backdrop(all, tint), Backdrop::Mica { chrome: 166, side: 191 });
        for s in [
            BackdropSupport { supported: false, ..all },
            BackdropSupport { transparency: false, ..all },
            BackdropSupport { high_contrast: true, ..all },
            BackdropSupport::default(),
        ] {
            assert_eq!(backdrop(s, tint), Backdrop::Solid, "{s:?}");
        }
        assert_eq!(backdrop(all, None), Backdrop::Solid, "an unreadable palette");
    }

    #[test]
    fn mica_needs_build_22621() {
        for (build, ok) in [(None, false), (Some(19045), false), (Some(22000), false), (Some(22621), true), (Some(26100), true)] {
            assert_eq!(mica_build(build), ok, "{build:?}");
        }
    }
}
