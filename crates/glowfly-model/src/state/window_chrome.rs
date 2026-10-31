//! The window frame on each OS and the pure parts of a title bar Glowfly draws itself: which
//! frame applies (`Chrome`, with the Windows system title bar setting), the window actions the bar asks for and full screen (`WindowCmd`),
//! what the shell
//! knows about the window (`WindowInfo`, with the Maximize button as Windows tracks it; full
//! screen narrows what a `WindowCmd` does there, `WindowCmd::in_window`) and the
//! Alt tap that opens the bar's menu (`AltTap`). The views (`ui/title_bar.rs`,
//! `ui/window_frame.rs`) turn these into widgets, and the shell turns `WindowCmd` into viewport
//! commands, or into the system menu through `platform::native_window`.

use super::window_native::CaptionButton;
use glowfly_core::opens::Os;

/// The room left at the start of the macOS bar for the traffic lights.
const TRAFFIC_LIGHTS_W: f32 = 78.0;

/// Who draws the window's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Chrome {
    /// The system frame and title bar, with Glowfly's menu bar and tab bar under it (Linux,
    /// and every test that doesn't pick another).
    #[default]
    System,
    /// No system frame: Glowfly draws the title bar, the window buttons and the resize edges.
    Windows,
    /// The system frame with a transparent, full-size title bar: Glowfly's bar sits under the
    /// traffic lights.
    MacOs,
}

impl Chrome {
    const fn for_os(os: Os) -> Chrome {
        match os {
            Os::Windows => Chrome::Windows,
            Os::MacOs => Chrome::MacOs,
            Os::Linux => Chrome::System,
        }
    }

    /// The frame of the OS Glowfly was built for.
    pub const fn current() -> Chrome {
        Self::for_os(Os::current())
    }

    /// The frame for `os` when the Windows system title bar setting is `system_title_bar`: the
    /// system's frame on Windows when it is on; other OSes ignore it.
    pub const fn chosen(os: Os, system_title_bar: bool) -> Chrome {
        match Self::for_os(os) {
            Chrome::Windows if system_title_bar => Chrome::System,
            c => c,
        }
    }

    /// Whether Preferences offers the system title bar setting: where the OS's frame is the one
    /// Glowfly draws on Windows.
    pub const fn offers_system_title_bar(os: Os) -> bool {
        matches!(Self::for_os(os), Chrome::Windows)
    }

    /// Whether Glowfly draws the title bar (with the menu button and the tabs).
    pub const fn drawn(self) -> bool {
        matches!(self, Chrome::Windows | Chrome::MacOs)
    }

    /// Whether the OS menu bar shows Glowfly's menus (`platform::native_menu`).
    pub const fn native_menu(self) -> bool {
        matches!(self, Chrome::MacOs)
    }

    /// Whether Glowfly draws Minimize, Maximize and Close.
    pub const fn window_buttons(self) -> bool {
        matches!(self, Chrome::Windows)
    }

    /// Whether Glowfly provides the resize edges and corners.
    pub const fn resize_edges(self) -> bool {
        matches!(self, Chrome::Windows)
    }

    /// Whether a tap of Alt opens the title bar's menu.
    pub const fn alt_opens_menu(self) -> bool {
        matches!(self, Chrome::Windows)
    }

    /// Whether the menus answer to Alt+letter mnemonics and show their letters: not on macOS,
    /// where Option+letter types characters and menus have none.
    pub const fn mnemonics(self) -> bool {
        !matches!(self, Chrome::MacOs)
    }

    /// Whether a right-click on the empty bar opens the window's system menu (Alt+Space opens
    /// it too, in the OS glue).
    pub const fn system_menu(self) -> bool {
        matches!(self, Chrome::Windows)
    }

    /// The room left empty at the bar's start: the traffic lights on macOS, which hide in
    /// fullscreen.
    pub fn leading_inset(self, fullscreen: bool) -> f32 {
        if matches!(self, Chrome::MacOs) && !fullscreen { TRAFFIC_LIGHTS_W } else { 0.0 }
    }
}

/// A window edge or corner to resize from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// A window action asked for by a drawn title bar, or by the full-screen key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCmd {
    /// Move the window with the OS's move loop (the primary button is down).
    StartDrag,
    /// Resize from an edge or corner with the OS's resize loop.
    BeginResize(ResizeEdge),
    Minimize,
    /// Maximize (true) or restore (false).
    SetMaximized(bool),
    /// Show the window's system menu at the pointer: a right-click on the empty bar.
    SystemMenu,
    /// Enter (true) or leave (false) full screen: the full-screen key, palette entry and View
    /// item.
    SetFullscreen(bool),
}

impl WindowCmd {
    /// What `self` does to a window in state `w`: a full-screen window can't be moved or
    /// resized, and Maximize or Restore leaves full screen.
    pub fn in_window(self, w: WindowInfo) -> Option<WindowCmd> {
        if !w.fullscreen {
            return Some(self);
        }
        match self {
            WindowCmd::StartDrag | WindowCmd::BeginResize(_) => None,
            WindowCmd::SetMaximized(_) => Some(WindowCmd::SetFullscreen(false)),
            WindowCmd::Minimize | WindowCmd::SystemMenu | WindowCmd::SetFullscreen(_) => Some(self),
        }
    }
}

/// What the shell read about the window this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowInfo {
    pub maximized: bool,
    pub fullscreen: bool,
    /// The Maximize button's hover and press while Windows tracks it for Snap Layouts (egui
    /// sees no pointer there then); default elsewhere.
    pub max_button: CaptionButton,
}

/// One frame's input, as far as an Alt tap cares.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AltFrame {
    /// Alt is held.
    pub alt: bool,
    /// Ctrl, Shift or Command is held (AltGr reads as Ctrl+Alt).
    pub other_modifiers: bool,
    /// A key went down, text was typed, a button was pressed or the wheel turned.
    pub other_input: bool,
    /// The window has the keyboard.
    pub focused: bool,
}

/// Tracks Alt pressed and released alone: no other key, text, click or scroll, no Ctrl, Shift
/// or Command, and the window kept the focus throughout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AltTap {
    held: bool,
    alone: bool,
}

impl AltTap {
    /// The tracker after frame `f`, and whether that frame finished a tap.
    pub fn step(self, f: AltFrame) -> (AltTap, bool) {
        if !f.focused {
            return (AltTap::default(), false);
        }
        let clean = !f.other_modifiers && !f.other_input;
        match (self.held, f.alt) {
            (false, true) => (AltTap { held: true, alone: clean }, false),
            (true, true) => (AltTap { held: true, alone: self.alone && clean }, false),
            (true, false) => (AltTap::default(), self.alone && !f.other_input),
            (false, false) => (AltTap::default(), false),
        }
    }

    /// The tracker once something else used the Alt press (the system menu opened): the
    /// release that follows never fires, whether or not Alt still reads as held.
    pub fn cancel(self) -> AltTap {
        AltTap { held: true, alone: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_os_gets_its_frame() {
        assert_eq!(Chrome::for_os(Os::Windows), Chrome::Windows);
        assert_eq!(Chrome::for_os(Os::MacOs), Chrome::MacOs);
        assert_eq!(Chrome::for_os(Os::Linux), Chrome::System);
        assert_eq!(Chrome::current(), Chrome::for_os(Os::current()));
        assert_eq!(Chrome::default(), Chrome::System);
    }

    #[test]
    fn the_system_title_bar_setting_only_changes_windows() {
        assert_eq!(Chrome::chosen(Os::Windows, false), Chrome::Windows);
        assert_eq!(Chrome::chosen(Os::Windows, true), Chrome::System);
        for on in [false, true] {
            assert_eq!(Chrome::chosen(Os::MacOs, on), Chrome::MacOs, "{on}");
            assert_eq!(Chrome::chosen(Os::Linux, on), Chrome::System, "{on}");
        }
        assert!(Chrome::offers_system_title_bar(Os::Windows));
        assert!(!Chrome::offers_system_title_bar(Os::MacOs) && !Chrome::offers_system_title_bar(Os::Linux));
    }

    #[test]
    fn what_each_frame_draws() {
        // (chrome, drawn, window buttons, resize edges, Alt opens the menu, system menu)
        for (c, drawn, buttons, edges, alt, menu) in [
            (Chrome::System, false, false, false, false, false),
            (Chrome::Windows, true, true, true, true, true),
            (Chrome::MacOs, true, false, false, false, false),
        ] {
            assert_eq!(c.drawn(), drawn, "{c:?}");
            assert_eq!(c.window_buttons(), buttons, "{c:?}");
            assert_eq!(c.resize_edges(), edges, "{c:?}");
            assert_eq!(c.alt_opens_menu(), alt, "{c:?}");
            assert_eq!(c.system_menu(), menu, "{c:?}");
            assert_eq!(c.mnemonics(), c != Chrome::MacOs, "{c:?}");
        }
    }

    #[test]
    fn native_menu_is_macos_only() {
        assert!(Chrome::for_os(Os::MacOs).native_menu());
        assert!(!Chrome::for_os(Os::Windows).native_menu());
        assert!(!Chrome::for_os(Os::Linux).native_menu());
    }

    #[test]
    fn only_macos_leaves_room_for_the_traffic_lights() {
        assert_eq!(Chrome::MacOs.leading_inset(false), 78.0);
        assert_eq!(Chrome::MacOs.leading_inset(true), 0.0, "the lights hide in fullscreen");
        for c in [Chrome::System, Chrome::Windows] {
            assert_eq!(c.leading_inset(false), 0.0);
            assert_eq!(c.leading_inset(true), 0.0);
        }
    }

    #[test]
    fn full_screen_blocks_moves_and_turns_maximize_into_leaving() {
        use WindowCmd as W;
        let windowed = WindowInfo::default();
        let full = WindowInfo { fullscreen: true, ..WindowInfo::default() };
        // (command, what it does in full screen)
        for (cmd, in_full) in [
            (W::StartDrag, None),
            (W::BeginResize(ResizeEdge::SouthEast), None),
            (W::BeginResize(ResizeEdge::North), None),
            (W::Minimize, Some(W::Minimize)),
            (W::SetMaximized(true), Some(W::SetFullscreen(false))),
            (W::SetMaximized(false), Some(W::SetFullscreen(false))),
            (W::SystemMenu, Some(W::SystemMenu)),
            (W::SetFullscreen(true), Some(W::SetFullscreen(true))),
            (W::SetFullscreen(false), Some(W::SetFullscreen(false))),
        ] {
            assert_eq!(cmd.in_window(windowed), Some(cmd), "{cmd:?} windowed");
            let maximized = WindowInfo { maximized: true, ..windowed };
            assert_eq!(cmd.in_window(maximized), Some(cmd), "{cmd:?} maximized");
            assert_eq!(cmd.in_window(full), in_full, "{cmd:?} in full screen");
        }
    }

    const UP: AltFrame = AltFrame { alt: false, other_modifiers: false, other_input: false, focused: true };
    const ALT: AltFrame = AltFrame { alt: true, ..UP };

    /// Runs `frames` from a fresh tracker; returns the frames (by index) that fired.
    fn fires(frames: &[AltFrame]) -> Vec<usize> {
        let mut tap = AltTap::default();
        let mut out = Vec::new();
        for (i, f) in frames.iter().enumerate() {
            let (next, fired) = tap.step(*f);
            tap = next;
            if fired {
                out.push(i);
            }
        }
        out
    }

    #[test]
    fn a_press_and_release_fires_once() {
        assert_eq!(fires(&[UP, ALT, ALT, UP, UP]), [3]);
    }

    #[test]
    fn alt_with_a_key_click_or_ctrl_does_not_fire() {
        let key = AltFrame { other_input: true, ..ALT };
        assert!(fires(&[ALT, key, ALT, UP]).is_empty(), "Alt+key");
        let ctrl = AltFrame { other_modifiers: true, ..ALT };
        assert!(fires(&[ctrl, ctrl, UP]).is_empty(), "Ctrl+Alt (AltGr)");
        assert!(fires(&[ALT, ctrl, ALT, UP]).is_empty(), "Ctrl pressed during the hold");
        let click = AltFrame { other_input: true, ..ALT };
        assert!(fires(&[ALT, click, UP]).is_empty(), "a click while held");
        assert!(fires(&[ALT, AltFrame { other_input: true, ..UP }]).is_empty(), "a key in the release frame");
    }

    #[test]
    fn losing_the_focus_resets_the_tap() {
        let away = AltFrame { focused: false, ..ALT };
        assert!(fires(&[ALT, away, UP]).is_empty());
        assert!(fires(&[ALT, AltFrame { focused: false, ..UP }]).is_empty());
    }

    #[test]
    fn alt_tap_cancel_swallows_the_release() {
        let (tap, _) = AltTap::default().step(ALT);
        let (tap, fired) = tap.cancel().step(UP);
        assert!(!fired, "the release after the system menu");
        assert_eq!(tap, AltTap::default());
        // Cancelled with Alt already up: the next tap still fires, once, on its release.
        let mut tap = AltTap::default().cancel();
        let mut out = Vec::new();
        for f in [UP, ALT, UP] {
            let (next, fired) = tap.step(f);
            tap = next;
            out.push(fired);
        }
        assert_eq!(out, [false, false, true]);
    }

    #[test]
    fn two_taps_fire_twice_and_a_hold_never_does() {
        assert_eq!(fires(&[ALT, UP, ALT, UP]), [1, 3]);
        assert!(fires(&[ALT; 20]).is_empty());
    }
}
