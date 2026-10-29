//! One Glowfly window for links and folders: a second `glowfly glowfly://…` or `glowfly <path>`
//! hands what it was given to the window already running instead of opening another.
//!
//! Which window that is: the one holding the lock on [`LOCK_NAME`] in the runtime folder
//! (`claim.rs`). Every window tries to take it at start-up ([`claim`]); one that finds it held
//! waits for it on a thread and takes over when the other window closes, so exactly one window
//! listens at a time, and a crashed window's lock is gone with its process.
//!
//! The window holding it listens on a loopback port ([`serve`]) and writes where, with a random
//! token and its process id, to a private file in the runtime folder ([`FILE_NAME`] under
//! `settings::runtime_dir`, mode 0600 on Unix; Windows keeps that folder private to its user).
//! Only a process that can read the file knows the token, so only this user's processes are
//! heard. A second process reads the file ([`hand_off`]), lets the first take the foreground
//! (Windows refuses focus to a window its user didn't just click otherwise), connects, and sends
//! one JSON line, `{"token", "open": <link>}` or `{"token", "folder": <absolute path>}`; the
//! listener answers `{"ok": true}` when the token matches and the item is well-formed, and
//! reports it. Lines are at most [`MAX_LINE`] bytes, and nothing in one is run: a link is read
//! like any other link and a folder opens like one picked in the app.
//!
//! No running window, a file left by one that crashed (its port is closed), a refusal, or a
//! window that doesn't answer in time: the second process opens its own window.

mod claim;
mod client;
mod file;
mod foreground;
mod server;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_claim;

use std::path::{Path, PathBuf};
use std::time::Duration;

pub use claim::{Claim, Claimed, claim, claim_at};
pub use client::{HandOffError, hand_off, hand_off_via, hand_off_waiting};
pub use server::{Listener, serve, serve_at};

/// The file that says where the running window listens.
pub const FILE_NAME: &str = "instance.json";
/// The file whose lock the running window holds, next to [`FILE_NAME`].
pub const LOCK_NAME: &str = "instance.lock";
/// How long either side waits for the other's line.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a second process tries to reach the port the file names.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// How long a second process keeps trying a window that holds the lock but isn't listening yet
/// (it is still starting).
pub const START_WAIT: Duration = Duration::from_secs(3);
/// The longest line either side reads.
const MAX_LINE: u64 = 16 * 1024;

/// Where the instance file lives: the runtime folder.
pub fn default_file() -> Option<PathBuf> {
    crate::settings::runtime_dir().map(|d| d.join(FILE_NAME))
}

/// What a second process hands over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handed {
    /// A `glowfly:` link.
    Link(String),
    /// A repository folder, absolute.
    Folder(PathBuf),
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Request {
    token: String,
    /// A link. Windows from before folders were handed over read only this field, so a folder
    /// sent to one is malformed there and the sender opens its own window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    open: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    folder: Option<String>,
}

impl Request {
    fn new(token: String, item: &Handed) -> std::io::Result<Self> {
        let (open, folder) = match item {
            Handed::Link(t) => (Some(t.clone()), None),
            Handed::Folder(p) => {
                let text = p.to_str().filter(|s| folder_from_wire(s).is_some()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "only an absolute path in Unicode can be handed over")
                })?;
                (None, Some(text.to_owned()))
            }
        };
        Ok(Self { token, open, folder })
    }

    /// The item, when the request names exactly one and it is well-formed.
    fn item(self) -> Option<Handed> {
        match (self.open, self.folder) {
            (Some(link), None) => crate::link::looks_like_link(std::ffi::OsStr::new(&link)).then_some(Handed::Link(link)),
            (None, Some(folder)) => folder_from_wire(&folder).map(Handed::Folder),
            _ => None,
        }
    }
}

/// The folder a request names: an absolute path without NUL characters (which no file system
/// allows).
fn folder_from_wire(text: &str) -> Option<PathBuf> {
    if text.contains('\0') {
        return None;
    }
    Some(Path::new(text)).filter(|p| p.is_absolute()).map(Path::to_path_buf)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Response {
    ok: bool,
}
