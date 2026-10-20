//! Effects that need the window, carried out by the UI shell (`Effect::Ui`).

use crate::state::window_chrome::WindowCmd;

/// Effects that need the window, carried out by the UI shell.
#[derive(Debug, PartialEq)]
pub enum UiEffect {
    CopyText(String),
    SetTitle(String),
    /// A credential prompt opened: flash the taskbar entry if the window isn't focused.
    RequestAttention,
    /// A link arrived: un-minimize, raise and focus the window (or flash it when that fails).
    FocusWindow,
    /// File > Quit: close the main window.
    CloseWindow,
    /// Read the clipboard into an Event::Paste for the focused widget on the next frame.
    RequestPaste,
    /// Move, resize, minimise or maximise the window: the drawn title bar.
    Window(WindowCmd),
}
