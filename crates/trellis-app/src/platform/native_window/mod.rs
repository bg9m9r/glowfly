//! The drawn Windows frame's glue with the OS: a window procedure in front of winit's
//! (`win_proc.rs`) that answers Windows' hit test with the Maximize button, so Windows 11 shows
//! Snap Layouts over it, opens the system menu on Alt+Space or on request, and reads whether
//! the Mica backdrop applies (`win_system.rs`). Only the shell (`ui/app.rs`, `ui/app_native.rs`)
//! talks to it: each frame it polls a `NativeFrame` and hands back the caption geometry the
//! views recorded. Elsewhere, and with another frame, nothing is installed.

// Outside Windows only the facade's signatures are used.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::state::window_chrome::Chrome;
use crate::state::window_native::{BackdropSupport, CaptionButton, CaptionGeometry};

#[cfg(windows)]
mod win_proc;
#[cfg(windows)]
mod win_system;

/// What the OS reported since the last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeFrame {
    /// The Maximize button as Windows tracks it.
    pub max_button: CaptionButton,
    /// A full click on the Maximize button.
    pub max_clicked: bool,
    /// The system menu was shown (it took the Alt press of Alt+Space).
    pub system_menu_shown: bool,
    pub support: BackdropSupport,
}

/// Whether this Windows can show Mica behind the window (build 22621 or later).
pub fn mica_capable() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
        let build =
            trellis_core::win_registry::string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber");
        crate::state::window_native::mica_build(build.and_then(|b| b.trim().parse().ok()))
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// The installed glue for the main window.
#[cfg(windows)]
pub struct NativeWindow {
    hwnd: isize,
    /// The dark mode last handed to DWM.
    last_dark: Option<bool>,
}

/// Never made outside Windows.
#[cfg(not(windows))]
pub enum NativeWindow {}

impl NativeWindow {
    /// Installs the glue on `cc`'s window: only with the drawn Windows frame. `mica` asks for
    /// the Mica backdrop (the viewport was made transparent for it); `ctx` is woken on every
    /// change.
    #[cfg(windows)]
    pub fn install(cc: &eframe::CreationContext<'_>, chrome: Chrome, mica: bool, ctx: eframe::egui::Context) -> Option<NativeWindow> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if chrome != Chrome::Windows {
            return None;
        }
        let RawWindowHandle::Win32(h) = cc.window_handle().ok()?.as_raw() else { return None };
        let hwnd = h.hwnd.get();
        if !win_proc::install(hwnd, mica, ctx) {
            tracing::warn!("could not hook the window procedure: no Snap Layouts or system menu");
            return None;
        }
        Some(NativeWindow { hwnd, last_dark: None })
    }

    #[cfg(not(windows))]
    pub fn install(_cc: &eframe::CreationContext<'_>, _chrome: Chrome, _mica: bool, _ctx: eframe::egui::Context) -> Option<NativeWindow> {
        None
    }

    /// What happened since the last poll; the click and the menu are reported once.
    pub fn poll(&mut self) -> NativeFrame {
        #[cfg(windows)]
        {
            win_proc::poll()
        }
        #[cfg(not(windows))]
        {
            match *self {}
        }
    }

    /// Where the caption parts are this frame (None: nothing to hit-test).
    pub fn set_geometry(&self, g: Option<CaptionGeometry>) {
        #[cfg(windows)]
        {
            win_proc::set_geometry(g);
        }
        #[cfg(not(windows))]
        {
            let _ = g;
            match *self {}
        }
    }

    /// Picks dark or light Mica (and DWM's own fallback) to match the palette.
    pub fn set_dark(&mut self, dark: bool) {
        #[cfg(windows)]
        {
            if self.last_dark != Some(dark) {
                win_system::set_dark(self.hwnd, dark);
                self.last_dark = Some(dark);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = dark;
            match *self {}
        }
    }

    /// Opens the system menu at the pointer, once the window procedure gets to it (never inside
    /// a frame: the menu runs its own message loop).
    pub fn show_system_menu_at_cursor(&self) {
        #[cfg(windows)]
        {
            win_proc::post_system_menu(self.hwnd);
        }
        #[cfg(not(windows))]
        {
            match *self {}
        }
    }
}
