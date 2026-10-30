//! The Windows calls behind the drawn frame: the frame shadow and Windows 11's rounded corners,
//! the Mica backdrop and its dark or light mode, the settings that turn it off (transparency
//! effects, high contrast), and the system menu.

use crate::state::window_native::{SysItem, system_menu_items};
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GetCursorPos, GetSystemMenu, IsZoomed, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED, PostMessageW, SC_CLOSE, SC_MAXIMIZE,
    SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu, WM_SYSCOMMAND,
};
use windows::core::BOOL;

/// Gives the frameless window what the system frame has: DWM's shadow, which it draws once the
/// frame reaches 1 px into the client area (Glowfly paints over that pixel), and on Windows 11
/// rounded corners (Windows 10 doesn't know the attribute and keeps square ones). Windows itself
/// squares the corners and drops the shadow while the window is maximized.
pub(super) fn set_frame_look(hwnd: HWND) {
    let margins = MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: 1, cyBottomHeight: 1 };
    // SAFETY: `margins` is a MARGINS that outlives the call, for our own live window.
    if let Err(e) = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) } {
        tracing::debug!("could not give the window a shadow: {e}");
    }
    let round = DWMWCP_ROUND;
    // SAFETY: `round` is a DWM_WINDOW_CORNER_PREFERENCE, the size passed, and outlives the call.
    let r = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const round).cast::<c_void>(),
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
    if let Err(e) = r {
        tracing::debug!("no rounded corners (before Windows 11): {e}");
    }
}

/// Asks DWM for Mica behind the window; false where Windows doesn't support it (before build
/// 22621 the attribute is unknown and the call fails).
pub(super) fn set_backdrop(hwnd: HWND) -> bool {
    let kind = DWMSBT_MAINWINDOW;
    // SAFETY: `kind` is a DWM_SYSTEMBACKDROP_TYPE, the size passed, and outlives the call.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const kind).cast::<c_void>(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .is_ok()
}

/// Dark or light Mica, and DWM's own fallback colour, to match the palette.
pub(super) fn set_dark(hwnd: isize, dark: bool) {
    let on = BOOL::from(dark);
    // SAFETY: `on` is a BOOL, the size passed, and outlives the call.
    let r = unsafe {
        DwmSetWindowAttribute(
            HWND(hwnd as *mut c_void),
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const on).cast::<c_void>(),
            std::mem::size_of::<BOOL>() as u32,
        )
    };
    if let Err(e) = r {
        tracing::debug!("could not set the window's dark mode: {e}");
    }
}

/// Settings > Personalization > Colors > Transparency effects (on when the value is missing).
pub(super) fn transparency_on() -> bool {
    let key = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    glowfly_core::win_registry::dword(HKEY_CURRENT_USER, key, "EnableTransparency").is_none_or(|v| v != 0)
}

/// Whether a Windows high-contrast theme is on.
pub(super) fn high_contrast_on() -> bool {
    let size = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    let mut hc = HIGHCONTRASTW { cbSize: size, ..Default::default() };
    // SAFETY: `hc` is a HIGHCONTRASTW with its size set, writable for the call.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, size, Some((&raw mut hc).cast::<c_void>()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
    }
    .is_ok();
    ok && (hc.dwFlags & HCF_HIGHCONTRASTON) == HCF_HIGHCONTRASTON
}

/// Where the system menu opens.
pub(super) enum MenuAt {
    /// A client point (below the title bar's menu button).
    Anchor(i32, i32),
    /// The pointer.
    Cursor,
}

fn command(item: SysItem) -> u32 {
    match item {
        SysItem::Restore => SC_RESTORE,
        SysItem::Move => SC_MOVE,
        SysItem::Size => SC_SIZE,
        SysItem::Minimize => SC_MINIMIZE,
        SysItem::Maximize => SC_MAXIMIZE,
        SysItem::Close => SC_CLOSE,
    }
}

/// Shows the window's system menu at `at`, its items enabled as the window allows, and posts
/// the chosen command as WM_SYSCOMMAND for the default procedure to carry out. Runs the menu's
/// own message loop: call it only from the window procedure.
pub(super) fn show_system_menu(hwnd: HWND, at: MenuAt) {
    // SAFETY: `hwnd` is our live window; the menu handle is the system's copy for it.
    let menu = unsafe { GetSystemMenu(hwnd, false) };
    if menu.is_invalid() {
        return;
    }
    // SAFETY: reads our window's state.
    let maximized = unsafe { IsZoomed(hwnd) }.as_bool();
    for (item, on) in system_menu_items(maximized) {
        // SAFETY: `menu` is valid; an id the menu lacks is ignored.
        let _ = unsafe { EnableMenuItem(menu, command(item), MF_BYCOMMAND | if on { MF_ENABLED } else { MF_GRAYED }) };
    }
    let mut p = POINT::default();
    match at {
        MenuAt::Anchor(x, y) => {
            p = POINT { x, y };
            // SAFETY: `p` is a valid POINT for the call to write.
            let _ = unsafe { ClientToScreen(hwnd, &mut p) };
        }
        MenuAt::Cursor => {
            // SAFETY: `p` is a valid POINT for the call to write.
            if unsafe { GetCursorPos(&mut p) }.is_err() {
                return;
            }
        }
    }
    let flags = TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON;
    // SAFETY: `menu` and `hwnd` are valid; no exclusion rectangle is passed.
    let id = unsafe { TrackPopupMenu(menu, flags, p.x, p.y, None, hwnd, None) }.0;
    if id != 0 {
        // SAFETY: posting a message to our own window has no memory effects.
        let _ = unsafe { PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(id as usize), LPARAM(0)) };
    }
}
