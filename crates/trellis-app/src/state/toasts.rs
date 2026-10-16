//! Toast notifications: short messages in a corner of the window that expire on their own.
//! A toast can carry one action button (`ToastAction`), which sends `Msg::ToastAction`.

use super::AppState;
use crate::msg::Msg;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const TOAST_LIFETIME: Duration = Duration::from_secs(5);
/// How long a toast with an action button stays up, so there is time to click it.
pub const ACTION_TOAST_LIFETIME: Duration = Duration::from_secs(10);
/// The most toasts shown at once; a new one drops the oldest.
const TOAST_LIMIT: usize = 4;

pub struct Toast {
    pub ok: bool,
    pub text: String,
    pub at: Instant,
    /// Unique for the session, so a click names the toast it was on.
    pub id: u64,
    pub action: Option<ToastAction>,
}

/// A button under a toast's text.
pub struct ToastAction {
    pub label: String,
    pub msg: Box<Msg>,
    /// The repository the action is for: it runs only while that is the active tab.
    pub workdir: Option<PathBuf>,
}

impl Toast {
    pub fn lifetime(&self) -> Duration {
        if self.action.is_some() { ACTION_TOAST_LIFETIME } else { TOAST_LIFETIME }
    }
}

impl AppState {
    pub fn toast(&mut self, ok: bool, text: impl Into<String>, now: Instant) {
        self.push_toast(ok, text.into(), None, now);
    }

    /// A toast with an action button. Returns its id.
    pub fn toast_action(&mut self, ok: bool, text: impl Into<String>, action: ToastAction, now: Instant) -> u64 {
        self.push_toast(ok, text.into(), Some(action), now)
    }

    /// Gives the newest toast (one just pushed, such as a finished job's) the action `action`,
    /// so it stays up long enough to click. Nothing happens without a toast.
    pub fn set_last_toast_action(&mut self, action: ToastAction) {
        if let Some(t) = self.toasts.last_mut() {
            t.action = Some(action);
        }
    }

    fn push_toast(&mut self, ok: bool, text: String, action: Option<ToastAction>, now: Instant) -> u64 {
        self.toast_seq += 1;
        self.toasts.push(Toast { ok, text, at: now, id: self.toast_seq, action });
        if self.toasts.len() > TOAST_LIMIT {
            self.toasts.remove(0);
        }
        self.toast_seq
    }
}
