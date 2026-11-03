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

use super::AppState;
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
}

/// A button under a toast's text.
pub struct ToastAction {
    pub label: String,
    pub msg: Box<Msg>,
    /// The repository the action is for: it runs only while that is the active tab.
    pub workdir: Option<PathBuf>,
}

impl Toast {
    /// How long it stays up with `time` chosen: longer with a button, so there is time to click
    /// it. None never expires: "Until I close them", or a failure offering a fix or Details.
    pub fn lifetime(&self, time: ToastTime) -> Option<Duration> {
        let button = self.action.is_some() || self.details.is_some();
        if button && !self.ok { None } else { time.lifetime(button) }
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

    /// Shows a toast, unless one with the same words and kind is up: that one is shown again in
    /// place, its timer restarted (a held one stays held) and its buttons this call's, as a new
    /// one would have them. It keeps its id, so a screen reader doesn't announce it again, and
    /// its one activity line. Returns its id.
    fn push_toast(&mut self, ok: bool, text: String, action: Option<ToastAction>, now: Instant) -> u64 {
        self.toast_seq += 1;
        let text = super::logs::redacted(text);
        if let Some(t) = self.toasts.iter_mut().find(|t| t.ok == ok && t.text == text) {
            (t.shown, t.at, t.counted) = (self.toast_seq, now, Duration::ZERO);
            (t.action, t.alt, t.details) = (action, None, None);
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
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;

    const T: ToastTime = ToastTime::Standard;

    fn secs(s: f32) -> Duration {
        Duration::from_secs_f32(s)
    }

    fn state() -> (AppState, Instant) {
        let t0 = Instant::now();
        (AppState::new(Settings::default(), Vec::new(), t0), t0)
    }

    #[test]
    fn a_toast_shown_once_restarts_instead_of_stacking() {
        let (mut st, t0) = state();
        let action = || ToastAction { label: "Fix".into(), msg: Box::new(Msg::Tick(t0)), workdir: None };
        let first = st.toast_action_once(true, "Refused", action(), t0);
        let again = st.toast_action_once(true, "Refused", action(), t0 + secs(3.0));
        assert_eq!(st.toasts.len(), 1);
        assert!(again > first, "a new id, as a toast shown again");
        assert_eq!((st.toasts[0].id, st.toasts[0].at), (again, t0 + secs(3.0)));
        st.toast_action_once(false, "Refused", action(), t0);
        assert_eq!(st.toasts.len(), 2, "a failure with the same words is its own toast");
    }

    #[test]
    fn the_same_toast_twice_restarts_the_first() {
        let (mut st, t0) = state();
        let first = st.toast_action(false, "Push failed", action(), t0);
        st.toast(true, "Copied", t0);
        let later = t0 + secs(3.0);
        assert_eq!(st.toast_action(false, "Push failed", action(), later), first, "the same toast, its id kept");
        st.toast(true, "Push failed", later);
        let texts: Vec<(bool, &str)> = st.toasts.iter().map(|t| (t.ok, t.text.as_str())).collect();
        assert_eq!(texts, [(false, "Push failed"), (true, "Copied"), (true, "Push failed")], "a success with the same words is its own");
        let t = &st.toasts[0];
        assert_eq!((t.id, t.first, t.at, t.counted, t.details.is_none()), (first, first, later, Duration::ZERO, true));
        // A plain toast lasts five seconds: shown again at 3 s, it is still up at 7 s.
        st.toast(true, "Copied", later);
        assert_eq!(st.toasts.len(), 3);
        assert!(!st.toasts[1].expired(t0 + secs(7.0), T) && st.toasts[1].expired(t0 + secs(8.0), T), "its timer restarted");
        assert_eq!(st.newest_toast_mut().map(|t| t.text.clone()).as_deref(), Some("Copied"), "the newest, though not the last");
    }

    fn action() -> ToastAction {
        ToastAction { label: "Pull".into(), msg: Box::new(Msg::ClearLog), workdir: None }
    }

    #[test]
    fn age_and_opacity_follow_the_lifetime() {
        let (mut st, t0) = state();
        st.toast(true, "plain", t0);
        st.toast_action(true, "with a button", action(), t0);
        let (plain, act) = (&st.toasts[0], &st.toasts[1]);
        assert_eq!(plain.age(t0 + secs(3.9)), secs(3.9));
        assert_eq!(plain.opacity(t0 + secs(3.9), T), 1.0);
        assert!((plain.opacity(t0 + secs(4.5), T) - 0.5).abs() < 0.01);
        assert!(!plain.expired(t0 + secs(4.9), T));
        assert!(plain.expired(t0 + secs(5.0), T));
        assert!(!act.expired(t0 + secs(9.9), T), "ten seconds with a button");
        assert!(act.expired(t0 + secs(10.0), T));
        st.expire_toasts(t0 + secs(5.0));
        assert_eq!(st.toasts.len(), 1);
    }

    #[test]
    fn holding_stops_the_clock() {
        let (mut st, t0) = state();
        st.toast(true, "plain", t0);
        let id = st.toasts[0].id;
        st.hold_toast(Some(id), t0 + secs(3.0));
        assert_eq!(st.held_toast(), Some(id));
        let t = &st.toasts[0];
        assert!(!t.expired(t0 + secs(600.0), T));
        assert_eq!(t.age(t0 + secs(600.0)), secs(3.0));
        st.expire_toasts(t0 + secs(600.0));
        assert_eq!(st.toasts.len(), 1, "a held toast stays");
        let out = t0 + secs(600.0);
        st.hold_toast(None, out);
        assert_eq!(st.held_toast(), None);
        let t = &st.toasts[0];
        assert!(!t.expired(out + secs(1.9), T));
        assert!(t.expired(out + secs(2.0), T), "five seconds less the three used");
    }

    #[test]
    fn holding_a_fading_toast_brings_it_back() {
        let (mut st, t0) = state();
        st.toast(true, "plain", t0);
        let id = st.toasts[0].id;
        assert!(st.toasts[0].opacity(t0 + secs(4.6), T) < 1.0);
        st.hold_toast(Some(id), t0 + secs(4.6));
        let t = &st.toasts[0];
        assert_eq!(t.opacity(t0 + secs(4.6), T), 1.0);
        assert_eq!(t.age(t0 + secs(4.6)), t.lifetime(ToastTime::Standard).unwrap() - HOLD_GRACE);
        let out = t0 + secs(10.0);
        st.hold_toast(None, out);
        let t = &st.toasts[0];
        assert_eq!(t.opacity(out + secs(0.9), T), 1.0);
        let half = t.opacity(out + secs(1.5), T);
        assert!(half > 0.0 && half < 1.0, "{half}");
        assert!(t.expired(out + secs(2.0), T));
    }

    #[test]
    fn one_toast_is_held_at_a_time() {
        let (mut st, t0) = state();
        st.toast(true, "one", t0);
        st.toast(true, "two", t0);
        let (a, b) = (st.toasts[0].id, st.toasts[1].id);
        st.hold_toast(Some(a), t0 + secs(1.0));
        st.hold_toast(Some(b), t0 + secs(2.0));
        assert_eq!(st.held_toast(), Some(b));
        assert!(!st.toasts[0].held && st.toasts[0].at == t0 + secs(2.0), "the first is let go");
        st.hold_toast(Some(b), t0 + secs(3.0));
        assert_eq!(st.toasts[1].age(t0 + secs(9.0)), secs(2.0), "holding again changes nothing");
        st.hold_toast(Some(999), t0 + secs(4.0));
        assert_eq!(st.held_toast(), None, "no toast has that id");
    }

    #[test]
    fn the_limit_spares_the_held_toast() {
        let (mut st, t0) = state();
        for i in 0..4 {
            st.toast(true, format!("t{i}"), t0);
        }
        st.hold_toast(Some(st.toasts[0].id), t0);
        st.toast(true, "t4", t0);
        let texts: Vec<&str> = st.toasts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["t0", "t2", "t3", "t4"]);
        st.hold_toast(None, t0);
        st.toast(true, "t5", t0);
        let texts: Vec<&str> = st.toasts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["t2", "t3", "t4", "t5"], "with none held the oldest goes");
    }

    #[test]
    fn replacing_a_held_toast_keeps_it_held() {
        let (mut st, t0) = state();
        let id = st.toast_replacing(None, true, "Zoom 110%", t0);
        st.hold_toast(Some(id), t0 + secs(2.0));
        let later = t0 + secs(4.0);
        assert_eq!(st.toast_replacing(Some(id), true, "Zoom 120%", later), id);
        let t = &st.toasts[0];
        assert_eq!((t.text.as_str(), t.held, t.counted, t.at), ("Zoom 120%", true, Duration::ZERO, later));
    }
}
