//! The window's glue with the operating system, which only the shell and `main` use: the drawn
//! Windows frame (`native_window`), the app id an installed Windows copy takes before its window
//! opens (`claim_app_id`), the folders and links macOS hands Glowfly.app as Apple events
//! (`macos_events`, kept in the runner's `open_events` inbox), the macOS system menu bar showing
//! Glowfly's menus (`native_menu`, built in AppKit by `macos_menu`), the Linux PRIMARY selection
//! the terminal sets and pastes (`primary_selection`), and the system's fonts for Chinese,
//! Japanese and Korean (`system_fonts`), and the Windows keypad's Alt codes kept from the pane keys
//! (`numpad_keys`). The runner's side is `runner::platform`.

use crate::start_arg::StartArg;

mod app_id;
#[cfg(target_os = "macos")]
mod macos_events;
#[cfg(target_os = "macos")]
mod macos_menu;
pub mod native_menu;
pub mod native_window;
pub mod numpad_keys;
pub mod primary_selection;
pub mod system_fonts;
pub use app_id::claim as claim_app_id;

/// How eframe builds winit's event loop: on macOS without winit's default app menu
/// (`native_menu`), on Windows with the hook that tells the keypad's digits apart (`numpad_keys`).
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        Some(Box::new(|b| {
            b.with_msg_hook(numpad_keys::msg_hook);
        }))
    }
    #[cfg(target_os = "macos")]
    {
        Some(native_menu::event_loop_hook())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Starts taking the folders and links the OS hands over, before the window exists: they wait in
/// the runner's inbox until it attaches. `start` is what the command line already opens.
pub fn install_open_events(start: Option<&StartArg>) {
    crate::runner::platform::open_events::install(start);
    #[cfg(target_os = "macos")]
    macos_events::install();
}
