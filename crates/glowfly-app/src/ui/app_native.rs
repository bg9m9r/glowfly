//! The shell's side of the drawn Windows frame's OS glue (`platform::native_window`): before
//! each frame it reads what the OS reported (the Maximize button as Windows tracks it, a click
//! on it, the system menu, whether Mica applies) into view state and messages; after drawing it
//! hands the OS the caption geometry the views recorded.

use super::theme::color;
use super::window_frame;
use crate::msg::{AppMsg, Msg};
use crate::platform::native_window::{NativeFrame, NativeWindow};
use crate::state::AppState;
use crate::state::window_chrome::{WindowCmd, WindowInfo};
use crate::state::window_native::{self, Backdrop, MicaTint};
use eframe::egui::{self, Color32};

/// Takes the OS's news into `st` and `msgs`; `tint` is the palette's Mica tint.
pub(super) fn before_draw(native: &mut NativeWindow, st: &mut AppState, tint: Option<MicaTint>, msgs: &mut Vec<Msg>) {
    let f = native.poll();
    native.set_dark(color::is_dark());
    st.view.window.max_button = f.max_button;
    st.view.backdrop = window_native::backdrop(f.support, tint);
    msgs.extend(frame_msgs(&f, st.view.window));
    // The menu took the Alt press: its release mustn't open the title bar's menu.
    if f.system_menu_shown {
        st.view.alt_tap = st.view.alt_tap.cancel();
    }
}

/// Hands the OS this frame's caption geometry.
pub(super) fn after_draw(native: &NativeWindow, ctx: &egui::Context, st: &AppState) {
    native.set_geometry(window_frame::caption_geometry(ctx, st));
}

/// The messages `f` stands for: a click on the Maximize button maximizes or restores.
pub(super) fn frame_msgs(f: &NativeFrame, w: WindowInfo) -> Vec<Msg> {
    if f.max_clicked { vec![Msg::App(AppMsg::Window(WindowCmd::SetMaximized(!w.maximized)))] } else { Vec::new() }
}

/// The colour the frame is cleared to: transparent where Mica shows through, else `bg`.
pub(super) fn clear_rgba(b: Backdrop, bg: Color32) -> [f32; 4] {
    match b {
        Backdrop::Mica { .. } => [0.0; 4],
        Backdrop::Solid => bg.to_normalized_gamma_f32(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_msgs_turn_a_click_into_maximize_or_restore() {
        let click = NativeFrame { max_clicked: true, ..NativeFrame::default() };
        let restored = WindowInfo::default();
        let maximized = WindowInfo { maximized: true, ..WindowInfo::default() };
        let sent = |f: &NativeFrame, w| {
            frame_msgs(f, w).into_iter().map(|m| if let Msg::App(AppMsg::Window(c)) = m { Some(c) } else { None }).collect::<Vec<_>>()
        };
        assert_eq!(sent(&click, restored), [Some(WindowCmd::SetMaximized(true))]);
        assert_eq!(sent(&click, maximized), [Some(WindowCmd::SetMaximized(false))]);
        assert!(frame_msgs(&NativeFrame::default(), restored).is_empty());
        let hover_only =
            NativeFrame { max_button: window_native::CaptionButton { hovered: true, pressed: true }, ..NativeFrame::default() };
        assert!(frame_msgs(&hover_only, maximized).is_empty());
    }

    #[test]
    fn clear_rgba_is_transparent_only_on_mica() {
        let bg = Color32::from_rgb(20, 40, 60);
        assert_eq!(clear_rgba(Backdrop::Mica { chrome: 166, side: 191 }, bg), [0.0; 4]);
        assert_eq!(clear_rgba(Backdrop::Solid, bg), bg.to_normalized_gamma_f32());
        assert_eq!(clear_rgba(Backdrop::Solid, bg)[3], 1.0, "opaque");
    }
}
