//! The window procedure Glowfly puts in front of winit's (a plain `GWLP_WNDPROC` subclass, which
//! needs no common-controls manifest). It answers Windows' hit test with `HTMAXBUTTON` over the
//! drawn Maximize button, so Windows 11 shows Snap Layouts there, and with `HTTOP` over the thin
//! strip along its top, so the window resizes from there as from the rest of its top edge
//! (`CaptionGeometry::hit`). It tracks the button's hover, press and click itself, since egui
//! sees no pointer there then. It opens the system
//! menu on Alt+Space and on the shell's request, and re-reads the backdrop settings when they
//! change. Everything else goes on to winit.
//!
//! A panic here would abort the process, so nothing below can panic: the state shared with the
//! shell lives in atomics (the window procedure and egui run on the same thread, so `Relaxed`
//! is enough), with no `RefCell`, unwrap or indexing that can fail.

use super::{NativeFrame, win_system};
use crate::state::window_native::{BackdropSupport, CaptionButton, CaptionEvent, CaptionGeometry, CaptionHit, PxRect};
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU8, Ordering::Relaxed};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent, VK_CONTROL, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, HTMAXBUTTON, HTTOP, PostMessageW, SetWindowLongPtrW, WM_APP, WM_DWMCOMPOSITIONCHANGED,
    WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SETTINGCHANGE,
    WM_SYSCHAR, WM_SYSKEYDOWN, WM_THEMECHANGED, WNDPROC,
};

/// The shell's request to open the system menu at the pointer.
const WM_SYSTEM_MENU: u32 = WM_APP + 0x54;

/// The window, and the procedure that was there before ours.
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static PREV_PROC: AtomicIsize = AtomicIsize::new(0);
/// The Maximize button in client pixels: left, top, right, bottom.
static MAX_RECT: [AtomicI32; 4] = [AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0)];
/// The resize strip along the button's top, in pixels.
static TOP_EDGE: AtomicI32 = AtomicI32::new(0);
/// Where Alt+Space opens the system menu, in client pixels.
static MENU_AT: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];
/// The button's state: bit 0 hovered, bit 1 pressed.
static CAPTION: AtomicU8 = AtomicU8::new(0);
/// A full click on the button, not yet polled.
static MAX_CLICKED: AtomicBool = AtomicBool::new(false);
/// The system menu was shown, not yet polled.
static MENU_SHOWN: AtomicBool = AtomicBool::new(false);
/// Leaving the frame is being tracked (`TrackMouseEvent`).
static TRACKING: AtomicBool = AtomicBool::new(false);
static TRANSPARENCY: AtomicBool = AtomicBool::new(false);
static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);
/// Windows accepted the Mica backdrop.
static SUPPORTED: AtomicBool = AtomicBool::new(false);
/// Woken on every change the shell should see.
static REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut c_void)
}

/// Puts the procedure in front of `hwnd`'s; false when that fails or a window has it already.
pub(super) fn install(hwnd: isize, mica: bool, ctx: eframe::egui::Context) -> bool {
    if hwnd == 0 || HWND_RAW.compare_exchange(0, hwnd, Relaxed, Relaxed).is_err() {
        return false;
    }
    let _ = REPAINT.set(ctx);
    reread_settings();
    // `window_proc` matches WNDPROC and lives for the whole program.
    let ours: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = window_proc;
    // SAFETY: `hwnd` is the live main window, owned by this thread, and `ours` is a window
    // procedure that lives for the whole program.
    let prev = unsafe { SetWindowLongPtrW(hwnd_of(hwnd), GWLP_WNDPROC, ours as usize as isize) };
    if prev == 0 {
        HWND_RAW.store(0, Relaxed);
        return false;
    }
    PREV_PROC.store(prev, Relaxed);
    win_system::set_frame_look(hwnd_of(hwnd));
    if mica {
        SUPPORTED.store(win_system::set_backdrop(hwnd_of(hwnd)), Relaxed);
    }
    true
}

/// What happened since the last poll.
pub(super) fn poll() -> NativeFrame {
    NativeFrame {
        max_button: from_bits(CAPTION.load(Relaxed)),
        max_clicked: MAX_CLICKED.swap(false, Relaxed),
        system_menu_shown: MENU_SHOWN.swap(false, Relaxed),
        support: BackdropSupport {
            supported: SUPPORTED.load(Relaxed),
            transparency: TRANSPARENCY.load(Relaxed),
            high_contrast: HIGH_CONTRAST.load(Relaxed),
        },
    }
}

/// Records this frame's caption geometry for the hit test and Alt+Space.
pub(super) fn set_geometry(g: Option<CaptionGeometry>) {
    let r = g.map_or(PxRect::default(), |g| g.max_button);
    for (slot, v) in MAX_RECT.iter().zip([r.left, r.top, r.right, r.bottom]) {
        slot.store(v, Relaxed);
    }
    TOP_EDGE.store(g.map_or(0, |g| g.top_edge), Relaxed);
    if let Some(g) = g {
        MENU_AT[0].store(g.menu_at.0, Relaxed);
        MENU_AT[1].store(g.menu_at.1, Relaxed);
    }
}

/// Asks the procedure to open the system menu at the pointer.
pub(super) fn post_system_menu(hwnd: isize) {
    // SAFETY: posting a message to our own window has no memory effects.
    if let Err(e) = unsafe { PostMessageW(Some(hwnd_of(hwnd)), WM_SYSTEM_MENU, WPARAM(0), LPARAM(0)) } {
        tracing::debug!("could not ask for the system menu: {e}");
    }
}

/// The caption geometry last recorded.
fn geometry() -> CaptionGeometry {
    let [l, t, r, b] = &MAX_RECT;
    let max_button = PxRect { left: l.load(Relaxed), top: t.load(Relaxed), right: r.load(Relaxed), bottom: b.load(Relaxed) };
    CaptionGeometry { max_button, top_edge: TOP_EDGE.load(Relaxed), menu_at: (MENU_AT[0].load(Relaxed), MENU_AT[1].load(Relaxed)) }
}

fn from_bits(bits: u8) -> CaptionButton {
    CaptionButton { hovered: bits & 1 != 0, pressed: bits & 2 != 0 }
}

fn to_bits(b: CaptionButton) -> u8 {
    u8::from(b.hovered) | (u8::from(b.pressed) << 1)
}

fn repaint() {
    if let Some(ctx) = REPAINT.get() {
        ctx.request_repaint();
    }
}

/// Feeds `e` to the button's state; a finished click is kept for the next poll.
fn step(e: CaptionEvent) {
    let (next, clicked) = from_bits(CAPTION.load(Relaxed)).step(e);
    let changed = CAPTION.swap(to_bits(next), Relaxed) != to_bits(next);
    if clicked {
        MAX_CLICKED.store(true, Relaxed);
    }
    if changed || clicked {
        repaint();
    }
}

fn reread_settings() {
    TRANSPARENCY.store(win_system::transparency_on(), Relaxed);
    HIGH_CONTRAST.store(win_system::high_contrast_on(), Relaxed);
}

/// The client point of a hit test's screen-coordinate `lp` (signed 16-bit x and y).
fn client_point(hwnd: HWND, lp: LPARAM) -> POINT {
    let mut p = POINT { x: (lp.0 & 0xFFFF) as i16 as i32, y: ((lp.0 >> 16) & 0xFFFF) as i16 as i32 };
    // SAFETY: `p` is a valid POINT for the call to write.
    let _ = unsafe { ScreenToClient(hwnd, &mut p) };
    p
}

/// Whether a WM_SYSKEYDOWN or WM_SYSCHAR is Alt+Space: Space with Alt down (lParam bit 29) and
/// no Ctrl, which AltGr adds.
fn alt_space(wp: WPARAM, lp: LPARAM) -> bool {
    // SAFETY: GetKeyState only reads this thread's keyboard state.
    let ctrl = unsafe { GetKeyState(i32::from(VK_CONTROL.0)) } < 0;
    wp.0 == usize::from(VK_SPACE.0) && lp.0 & (1 << 29) != 0 && !ctrl
}

/// Key repeats set lParam bit 30.
fn repeat(lp: LPARAM) -> bool {
    lp.0 & (1 << 30) != 0
}

fn over_max(wp: WPARAM) -> bool {
    wp.0 == HTMAXBUTTON as usize
}

/// Starts the notice of the pointer leaving the frame, once.
fn track_leave(hwnd: HWND) {
    if TRACKING.swap(true, Relaxed) {
        return;
    }
    let mut t = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE | TME_NONCLIENT,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    // SAFETY: `t` is a properly sized TRACKMOUSEEVENT for our own window.
    if unsafe { TrackMouseEvent(&mut t) }.is_err() {
        TRACKING.store(false, Relaxed);
    }
}

/// Passes the message on to the procedure that was there before ours.
fn forward(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let prev = PREV_PROC.load(Relaxed);
    if prev == 0 {
        // SAFETY: the default procedure accepts any message for our window.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }
    // SAFETY: `prev` is the non-zero value GWLP_WNDPROC held before ours: a window procedure
    // (or a handle CallWindowProcW resolves), and WNDPROC is an Option of a function pointer,
    // the same size as isize.
    let prev = unsafe { std::mem::transmute::<isize, WNDPROC>(prev) };
    // SAFETY: the arguments are the ones Windows passed us, for the same window.
    unsafe { CallWindowProcW(prev, hwnd, msg, wp, lp) }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            let p = client_point(hwnd, lp);
            match geometry().hit(p.x, p.y) {
                Some(CaptionHit::Top) => return LRESULT(HTTOP as isize),
                Some(CaptionHit::MaxButton) => return LRESULT(HTMAXBUTTON as isize),
                None => {}
            }
        }
        WM_NCMOUSEMOVE => {
            step(CaptionEvent::Move { over: over_max(wp) });
            track_leave(hwnd);
            // Over the button the hover is ours: no caption-button tracking of the system's.
            if over_max(wp) {
                return LRESULT(0);
            }
        }
        WM_NCMOUSELEAVE => {
            TRACKING.store(false, Relaxed);
            step(CaptionEvent::Leave);
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if over_max(wp) => {
            step(CaptionEvent::Down { over: true });
            return LRESULT(0);
        }
        WM_NCLBUTTONUP => {
            step(CaptionEvent::Up { over: over_max(wp) });
            if over_max(wp) {
                return LRESULT(0);
            }
        }
        // Alt+Space never reaches winit, so egui sees no Space; the WM_SYSCHAR that follows
        // opens the menu.
        WM_SYSKEYDOWN if alt_space(wp, lp) => return LRESULT(0),
        WM_SYSCHAR if alt_space(wp, lp) => {
            if !repeat(lp) {
                let at = win_system::MenuAt::Anchor(MENU_AT[0].load(Relaxed), MENU_AT[1].load(Relaxed));
                open_menu(hwnd, at);
            }
            return LRESULT(0);
        }
        WM_SYSTEM_MENU => {
            open_menu(hwnd, win_system::MenuAt::Cursor);
            return LRESULT(0);
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DWMCOMPOSITIONCHANGED => {
            reread_settings();
            repaint();
        }
        WM_NCDESTROY => {
            let prev = PREV_PROC.load(Relaxed);
            if prev != 0 {
                // SAFETY: puts back the procedure that was there before ours, on the same window.
                unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, prev) };
            }
            HWND_RAW.store(0, Relaxed);
        }
        _ => {}
    }
    forward(hwnd, msg, wp, lp)
}

/// Shows the system menu at `at`. Marked shown first: a frame drawn while the menu runs its
/// loop already cancels the Alt tap.
fn open_menu(hwnd: HWND, at: win_system::MenuAt) {
    MENU_SHOWN.store(true, Relaxed);
    repaint();
    win_system::show_system_menu(hwnd, at);
}
