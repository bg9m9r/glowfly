//! Toast notifications: short messages in a corner of the window that expire on their own.
//! A toast can carry one action button (`ToastAction`), which sends `Msg::ToastAction`; a
//! failure with a fix (update/job_errors.rs) also carries Details beside it, which sends
//! `Msg::ToastDetails`, and may carry a second fix between them (`Msg::ToastAlt`). Every toast
//! has an x that closes it (`NotifyMsg::CloseToast`).
//!
//! How long a toast stays is Preferences > Accessibility's `toast_time`; a failure that offers a
//! fix or Details stays until it is closed. The pointer over a toast, or the keyboard focus in
//! it, holds it (`hold_toast`): its clock stops, so it neither fades nor expires, and when it is
//! let go at least `HOLD_GRACE` is left.
//!
//! A toast never stacks a copy of itself: one with the same words and kind as a toast that is up
//! restarts that toast's timer instead (`push_toast`), so clicking a refusal twice leaves one.
//! A toast carrying a fix is its own, though: two fixes with the same words may be for different
//! things (a message can't be compared), so both stay.

use super::AppState;
use crate::logs::LogsMsg;
use crate::msg::Msg;
use glowfly_core::settings::ToastTime;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The most toasts shown at once; a new one drops the oldest that isn't held.
const TOAST_LIMIT: usize = 4;
/// The least time a toast has left once the pointer leaves it.
const HOLD_GRACE: Duration = Duration::from_secs(2);
/// A toast fades out over its last second.
const FADE: Duration = Duration::from_secs(1);
/// How long a brief success stays under Standard.
const BRIEF: Duration = Duration::from_secs(3);

pub struct Toast {
    pub ok: bool,
    pub text: String,
    /// When its clock last started: shown, updated, or let go by the pointer.
    pub at: Instant,
    /// Time already counted before `at`; zero unless it was held.
    pub counted: Duration,
    /// The pointer is over it: its clock is stopped.
    pub held: bool,
    /// Unique for the session, so a click names the toast it was on.
    pub id: u64,
    /// Its id when it first showed: a toast shown again in place keeps it, so the activity log
    /// has one line for it.
    pub first: u64,
    /// `toast_seq` when it last showed or was shown again in place: the newest one is the
    /// toast an update just raised (update/logs_tabs.rs gives it Details).
    pub shown: u64,
    pub action: Option<ToastAction>,
    /// A second fix beside `action` (Try again beside Delete lock file).
    pub alt: Option<ToastAction>,
    /// Details, beside `action` when that holds a fix; a toast without a fix keeps Details in
    /// `action`.
    pub details: Option<ToastAction>,
    /// A success that says what just happened (a push or pull): with no button, it stays a
    /// shorter time under Standard (`BRIEF`).
    pub brief: bool,
}

/// A button under a toast's text.
pub struct ToastAction {
    pub label: String,
    pub msg: Box<Msg>,
    /// The repository the action is for: it runs only while that is the active tab.
    pub workdir: Option<PathBuf>,
}

impl Toast {
    /// It carries a fix: an action other than the activity log's Details, or a second fix.
    fn has_fix(&self) -> bool {
        let details = |a: &ToastAction| matches!(*a.msg, Msg::Logs(LogsMsg::Details { .. } | LogsMsg::Show(_)));
        self.alt.is_some() || self.action.as_ref().is_some_and(|a| !details(a))
    }

    /// How long it stays up with `time` chosen: longer with a button, so there is time to click
    /// it. None never expires: "Until I close them", or a failure offering a fix or Details.
    pub fn lifetime(&self, time: ToastTime) -> Option<Duration> {
        let button = self.action.is_some() || self.details.is_some();
        match (button, time) {
            (true, _) if !self.ok => None,
            (false, ToastTime::Standard) if self.brief => Some(BRIEF),
            _ => time.lifetime(button),
        }
    }

    /// How long it has been up at `now`, not counting the time it was held.
    pub fn age(&self, now: Instant) -> Duration {
        self.counted + if self.held { Duration::ZERO } else { now.saturating_duration_since(self.at) }
    }

    /// 1 until its last second, then down to 0.
    pub fn opacity(&self, now: Instant, time: ToastTime) -> f32 {
        let Some(life) = self.lifetime(time) else { return 1.0 };
        let fading = self.age(now).as_secs_f32() - life.saturating_sub(FADE).as_secs_f32();
        1.0 - fading.clamp(0.0, 1.0)
    }

    pub fn expired(&self, now: Instant, time: ToastTime) -> bool {
        !self.held && self.lifetime(time).is_some_and(|life| self.age(now) >= life)
    }
}

impl AppState {
    pub fn toast(&mut self, ok: bool, text: impl Into<String>, now: Instant) {
        self.push_toast(ok, text.into(), None, now);
    }

    /// A brief success toast (`Toast::brief`): a push or pull saying what it did.
    pub fn toast_brief(&mut self, text: impl Into<String>, now: Instant) {
        let id = self.push_toast(true, text.into(), None, now);
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.brief = true;
        }
    }

    /// A toast with an action button. Returns its id.
    pub fn toast_action(&mut self, ok: bool, text: impl Into<String>, action: ToastAction, now: Instant) -> u64 {
        self.push_toast(ok, text.into(), Some(action), now)
    }

    /// Gives the newest toast (one just pushed or shown again, such as a finished job's) the
    /// action `action`, so it stays up long enough to click. Nothing happens without a toast.
    pub fn set_last_toast_action(&mut self, action: ToastAction) {
        if let Some(t) = self.newest_toast_mut() {
            t.action = Some(action);
        }
    }

    /// The toast shown last: the one just pushed, or shown again in place (`push_toast`).
    pub fn newest_toast(&self) -> Option<&Toast> {
        self.toasts.iter().max_by_key(|t| t.shown)
    }

    /// `newest_toast`, to change.
    pub fn newest_toast_mut(&mut self) -> Option<&mut Toast> {
        self.toasts.iter_mut().max_by_key(|t| t.shown)
    }

    /// `toast_action`, unless a toast with the same words is up: that one is shown again in its
    /// place (a new id, its timer restarted, this action), so a refusal clicked again doesn't
    /// stack copies. Returns its id.
    pub fn toast_action_once(&mut self, ok: bool, text: impl Into<String>, action: ToastAction, now: Instant) -> u64 {
        let text = super::logs::redacted(text.into());
        let Some(at) = self.toasts.iter().position(|t| t.ok == ok && t.text == text) else {
            return self.push_toast(ok, text, Some(action), now);
        };
        self.toast_seq += 1;
        let t = &mut self.toasts[at];
        // A held toast stays held, its clock started again.
        (t.id, t.shown, t.at, t.counted, t.action) = (self.toast_seq, self.toast_seq, now, Duration::ZERO, Some(action));
        t.id
    }

    /// Updates the toast `previous` in place (its text, colour and timer) while it is still up,
    /// else shows a new one. Returns the toast's id, for the next call.
    pub fn toast_replacing(&mut self, previous: Option<u64>, ok: bool, text: impl Into<String>, now: Instant) -> u64 {
        let text = super::logs::redacted(text.into());
        if let Some(t) = previous.and_then(|id| self.toasts.iter_mut().find(|t| t.id == id)) {
            // A held toast stays held, its clock started again.
            (t.text, t.ok, t.at, t.counted) = (text, ok, now, Duration::ZERO);
            return t.id;
        }
        self.push_toast(ok, text, None, now)
    }

    /// Shows a toast, unless one with the same words and kind is up and neither carries a fix
    /// (`Toast::has_fix`; a fix's target can't be compared, so two with the same words both
    /// stay): that one is shown again in place, its timer restarted (a held one stays held) and
    /// its Details dropped, for the new announcement to give again. It keeps its id and its one
    /// activity line; a screen reader reads a failure shown again (`shown`), not a success.
    /// Returns its id.
    fn push_toast(&mut self, ok: bool, text: String, action: Option<ToastAction>, now: Instant) -> u64 {
        self.toast_seq += 1;
        let text = super::logs::redacted(text);
        let fix = action.as_ref().is_some_and(|a| !matches!(*a.msg, Msg::Logs(LogsMsg::Details { .. } | LogsMsg::Show(_))));
        if !fix && let Some(t) = self.toasts.iter_mut().find(|t| t.ok == ok && t.text == text && !t.has_fix()) {
            (t.shown, t.at, t.counted) = (self.toast_seq, now, Duration::ZERO);
            (t.action, t.details) = (action, None);
            return t.id;
        }
        let t = Toast {
            ok,
            text,
            at: now,
            counted: Duration::ZERO,
            held: false,
            id: self.toast_seq,
            first: self.toast_seq,
            shown: self.toast_seq,
            action,
            alt: None,
            details: None,
            brief: false,
        };
        self.toasts.push(t);
        if self.toasts.len() > TOAST_LIMIT {
            // The toast being read stays; the one shown longest ago goes.
            let oldest = self.toasts.iter().enumerate().filter(|(_, t)| !t.held).min_by_key(|(_, t)| t.shown).map_or(0, |(i, _)| i);
            self.toasts.remove(oldest);
        }
        self.toast_seq
    }

    /// The pointer is over toast `id` or the keyboard focus is in it, or neither is in any: holds
    /// that one and lets any other go. A fading toast comes back to full strength; one let go has
    /// at least `HOLD_GRACE` left.
    pub fn hold_toast(&mut self, id: Option<u64>, now: Instant) {
        let time = self.settings.a11y.toast_time;
        for t in self.toasts.iter_mut().filter(|t| Some(t.id) != id && t.held) {
            (t.at, t.held) = (now, false);
        }
        if let Some(t) = self.toasts.iter_mut().find(|t| Some(t.id) == id && !t.held) {
            let age = t.age(now);
            t.counted = t.lifetime(time).map_or(age, |life| age.min(life.saturating_sub(HOLD_GRACE)));
            t.held = true;
        }
    }

    /// The toast under the pointer.
    pub fn held_toast(&self) -> Option<u64> {
        self.toasts.iter().find(|t| t.held).map(|t| t.id)
    }

    /// Drops the toasts whose time is up at `now`.
    pub fn expire_toasts(&mut self, now: Instant) {
        let time = self.settings.a11y.toast_time;
        self.toasts.retain(|t| !t.expired(now, time));
    }
}

#[cfg(test)]
#[path = "toasts_tests.rs"]
mod tests;
