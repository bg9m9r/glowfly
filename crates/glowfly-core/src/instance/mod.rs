//! One Glowfly window for links: a second `glowfly glowfly://…` hands its link to the window
//! already running instead of opening another.
//!
//! The running app listens on a loopback port ([`serve`]) and writes where, with a random token
//! and its process id, to a private file in the runtime folder ([`FILE_NAME`] under
//! `settings::runtime_dir`, mode 0600 on Unix). A second process reads the file
//! ([`hand_off`]), lets the first take the foreground (Windows refuses focus to a window its
//! user didn't just click otherwise), connects, and sends one JSON line `{"token", "open"}`; the
//! listener answers `{"ok": true}` when the token matches and reports the link. This is the
//! scheme the credential prompts use (`askpass`), so every OS runs the same code.
//!
//! No running window, a file left by one that crashed (its port is closed), or a refusal: the
//! second process opens its own window.

mod client;
mod file;
mod foreground;
mod server;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::Duration;

pub use client::{HandOffError, hand_off, hand_off_via};
pub use server::{Listener, serve, serve_at};

/// The file that says where the running window listens.
pub const FILE_NAME: &str = "instance.json";
/// How long either side waits for the other's line.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a second process tries to reach the port the file names.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// The longest line either side reads.
const MAX_LINE: u64 = 16 * 1024;

/// Where the instance file lives: the runtime folder.
pub fn default_file() -> Option<PathBuf> {
    crate::settings::runtime_dir().map(|d| d.join(FILE_NAME))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Request {
    token: String,
    open: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Response {
    ok: bool,
}
