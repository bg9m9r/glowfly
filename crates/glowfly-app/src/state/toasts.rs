//! Toast notifications: short messages in a corner of the window that expire on their own.
//! A toast can carry one action button (`ToastAction`), which sends `Msg::ToastAction`; a
//! failure with a fix (update/job_errors.rs) also carries Details beside it, which sends
//! `Msg::ToastDetails`. Every toast has an x that closes it (`NotifyMsg::CloseToast`).
//!
//! The pointer over a toast holds it (`hold_toast`): its clock stops, so it neither fades nor
//! expires, and when the pointer leaves at least `HOLD_GRACE` is left.

use super::AppState;
use crate::msg::Msg;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const TOAST_LIFETIME: Duration = Duration::from_secs(5);
/// How long a toast with an action button stays up, so there is time to click it.
pub const ACTION_TOAST_LIFETIME: Duration = Duration::from_secs(10);
/// The most toasts shown at once; a new one drops the oldest that isn't held.
const TOAST_LIMIT: usize = 4;
/// The least time a toast has left once the pointer leaves it.
pub const HOLD_GRACE: Duration = Duration::from_secs(2);
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
    pub action: Option<ToastAction>,
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
    pub fn lifetime(&self) -> Duration {
        if self.action.is_some() || self.details.is_some() { ACTION_TOAST_LIFETIME } else { TOAST_LIFETIME }
    }

    /// How long it has been up at `now`, not counting the time it was held.
    pub fn age(&self, now: Instant) -> Duration {
        self.counted + if self.held { Duration::ZERO } else { now.saturating_duration_since(self.at) }
    }

    /// 1 until its last second, then down to 0.
    pub fn opacity(&self, now: Instant) -> f32 {
        let fading = self.age(now).as_secs_f32() - self.lifetime().saturating_sub(FADE).as_secs_f32();
        1.0 - fading.clamp(0.0, 1.0)
    }

    pub fn expired(&self, now: Instant) -> bool {
        !self.held && self.age(now) >= self.lifetime()
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

    fn push_toast(&mut self, ok: bool, text: String, action: Option<ToastAction>, now: Instant) -> u64 {
        self.toast_seq += 1;
        let text = super::logs::redacted(text);
        let t = Toast { ok, text, at: now, counted: Duration::ZERO, held: false, id: self.toast_seq, action, details: None };
        self.toasts.push(t);
        if self.toasts.len() > TOAST_LIMIT {
            // The toast being read stays.
            let oldest = self.toasts.iter().position(|t| !t.held).unwrap_or(0);
            self.toasts.remove(oldest);
        }
        self.toast_seq
    }

    /// The pointer is over toast `id`, or over none: holds that one and lets any other go. A
    /// fading toast comes back to full strength; one let go has at least `HOLD_GRACE` left.
    pub fn hold_toast(&mut self, id: Option<u64>, now: Instant) {
        for t in self.toasts.iter_mut().filter(|t| Some(t.id) != id && t.held) {
            (t.at, t.held) = (now, false);
        }
        if let Some(t) = self.toasts.iter_mut().find(|t| Some(t.id) == id && !t.held) {
            t.counted = t.age(now).min(t.lifetime().saturating_sub(HOLD_GRACE));
            t.held = true;
        }
    }

    /// The toast under the pointer.
    pub fn held_toast(&self) -> Option<u64> {
        self.toasts.iter().find(|t| t.held).map(|t| t.id)
    }

    /// Drops the toasts whose time is up at `now`.
    pub fn expire_toasts(&mut self, now: Instant) {
        self.toasts.retain(|t| !t.expired(now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;

    fn secs(s: f32) -> Duration {
        Duration::from_secs_f32(s)
    }

    fn state() -> (AppState, Instant) {
        let t0 = Instant::now();
        (AppState::new(Settings::default(), Vec::new(), t0), t0)
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
        assert_eq!(plain.opacity(t0 + secs(3.9)), 1.0);
        assert!((plain.opacity(t0 + secs(4.5)) - 0.5).abs() < 0.01);
        assert!(!plain.expired(t0 + secs(4.9)));
        assert!(plain.expired(t0 + secs(5.0)));
        assert!(!act.expired(t0 + secs(9.9)), "ten seconds with a button");
        assert!(act.expired(t0 + secs(10.0)));
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
        assert!(!t.expired(t0 + secs(600.0)));
        assert_eq!(t.age(t0 + secs(600.0)), secs(3.0));
        st.expire_toasts(t0 + secs(600.0));
        assert_eq!(st.toasts.len(), 1, "a held toast stays");
        let out = t0 + secs(600.0);
        st.hold_toast(None, out);
        assert_eq!(st.held_toast(), None);
        let t = &st.toasts[0];
        assert!(!t.expired(out + secs(1.9)));
        assert!(t.expired(out + secs(2.0)), "five seconds less the three used");
    }

    #[test]
    fn holding_a_fading_toast_brings_it_back() {
        let (mut st, t0) = state();
        st.toast(true, "plain", t0);
        let id = st.toasts[0].id;
        assert!(st.toasts[0].opacity(t0 + secs(4.6)) < 1.0);
        st.hold_toast(Some(id), t0 + secs(4.6));
        let t = &st.toasts[0];
        assert_eq!(t.opacity(t0 + secs(4.6)), 1.0);
        assert_eq!(t.age(t0 + secs(4.6)), t.lifetime() - HOLD_GRACE);
        let out = t0 + secs(10.0);
        st.hold_toast(None, out);
        let t = &st.toasts[0];
        assert_eq!(t.opacity(out + secs(0.9)), 1.0);
        let half = t.opacity(out + secs(1.5));
        assert!(half > 0.0 && half < 1.0, "{half}");
        assert!(t.expired(out + secs(2.0)));
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
