//! Tests of toast state: merging repeats, lifetimes, holding and the limit.

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
    let first = st.toast_action(false, "Push failed", details(), t0);
    st.toast(true, "Copied", t0);
    let later = t0 + secs(3.0);
    st.toast(false, "Push failed", later);
    st.toast(true, "Push failed", later);
    let texts: Vec<(bool, &str)> = st.toasts.iter().map(|t| (t.ok, t.text.as_str())).collect();
    assert_eq!(texts, [(false, "Push failed"), (true, "Copied"), (true, "Push failed")], "a success with the same words is its own");
    let t = &st.toasts[0];
    assert_eq!(
        (t.id, t.first, t.at, t.counted, t.action.is_none()),
        (first, first, later, Duration::ZERO, true),
        "its Details dropped for the new one"
    );
    // A plain toast lasts five seconds: shown again at 3 s, it is still up at 7 s.
    st.toast(true, "Copied", later);
    assert_eq!(st.toasts.len(), 3);
    assert!(!st.toasts[1].expired(t0 + secs(7.0), T) && st.toasts[1].expired(t0 + secs(8.0), T), "its timer restarted");
    assert_eq!(st.newest_toast().map(|t| t.text.as_str()), Some("Copied"), "the newest, though not the last");
}

#[test]
fn toasts_with_a_fix_never_merge() {
    let (mut st, t0) = state();
    let fix = |n: &str| ToastAction {
        label: "Undo".into(),
        msg: Box::new(Msg::Branch(crate::msg::BranchMsg::Checkout(n.into()))),
        workdir: None,
    };
    st.toast_action(true, "Checked out", fix("a1"), t0);
    st.toast_action(true, "Checked out", fix("b2"), t0);
    assert_eq!(st.toasts.len(), 2, "the same words, maybe for another branch: both stay");
    st.toast(true, "Checked out", t0);
    assert_eq!(st.toasts.len(), 3, "nor does a plain one merge into a fix");
    assert!(st.toasts.iter().take(2).all(Toast::has_fix) && !st.toasts[2].has_fix());
    st.toast(true, "Checked out", t0);
    assert_eq!(st.toasts.len(), 3, "two plain ones do");
}

fn details() -> ToastAction {
    let msg = Msg::Logs(LogsMsg::Show(crate::logs::LogKind::Activity));
    ToastAction { label: "Details".into(), msg: Box::new(msg), workdir: None }
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
