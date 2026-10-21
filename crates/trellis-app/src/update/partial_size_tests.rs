//! Headless tests for Fetch full history's size check: a large history asks first, a small or
//! unknown one fetches at once, and only a trusted repository asks its hosting service.

use super::shallow_tests::{cleanup, shallow_clone};
use super::tests::{drive, git, skipped};
use super::update;
use crate::effect::Effect;
use crate::effect::partial::PartialEffect;
use crate::msg::{JobMsg, Msg, PartialMsg, RepoMsg, TrustMsg};
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::forge::Provider;
use trellis_core::history_size::{HistorySize, SizeSource};
use trellis_core::shallow_ops::Deepen;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn full() -> Msg {
    Msg::Job(JobMsg::Deepen(Deepen::Full))
}

fn sized(st: &AppState, id: u64, size: HistorySize) -> Msg {
    Msg::Partial(PartialMsg::HistorySized { workdir: st.workdir().unwrap(), id, size })
}

fn unshallow(fx: &[Effect]) -> bool {
    let want = [trellis_core::gitcli::args(&["fetch", "--unshallow", "origin"])];
    matches!(fx, [Effect::RunGit { cmds, stoppable: true, .. }] if cmds[..] == want[..])
}

#[test]
fn a_large_history_asks_first() {
    let (mut st, src, dest) = shallow_clone("size-large", 3, 1);
    let fx = run(&mut st, full());
    assert!(matches!(&fx[..], [Effect::Partial(PartialEffect::SizeHistory(_))]));
    let id = st.partial.sizing.as_ref().unwrap().id;
    let big = || HistorySize::Known { bytes: 3 << 30, source: SizeSource::Service("GitHub".into()) };
    let other = sized(&st, id + 1, big());
    assert!(run(&mut st, other).is_empty(), "another check's answer");
    assert!(st.partial.sizing.is_some() && st.modal.is_none());

    let answer = sized(&st, id, big());
    assert!(run(&mut st, answer).is_empty());
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("asks first") };
    assert_eq!((title.as_str(), button.as_str()), ("Fetch full history?", "Fetch full history"));
    assert!(body.starts_with("GitHub reports this repository's size as about 3.0 GB. This clone has 1 commit so far;"), "{body}");
    assert!(matches!(**then, Msg::Job(JobMsg::DeepenNow(Deepen::Full))));
    assert!(st.partial.sizing.is_none() && st.job.is_none());

    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(unshallow(&fx), "the button fetches without asking again");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch full history"));
    assert!(st.job_cancellable());
    cleanup(&[&src, &dest]);
}

#[test]
fn a_small_or_unknown_history_fetches_at_once() {
    let (mut st, src, dest) = shallow_clone("size-small", 3, 1);
    run(&mut st, full());
    let id = st.partial.sizing.as_ref().unwrap().id;
    let small = sized(&st, id, HistorySize::Known { bytes: 4096, source: SizeSource::Folder("../src".into()) });
    assert!(unshallow(&run(&mut st, small)));
    assert!(st.modal.is_none());
    st.job = None;
    st.job_stop = None;

    run(&mut st, full());
    let id = st.partial.sizing.as_ref().unwrap().id;
    let unknown = sized(&st, id, HistorySize::Unknown);
    assert!(unshallow(&run(&mut st, unknown)), "nothing says it is large");
    cleanup(&[&src, &dest]);
}

#[test]
fn only_a_trusted_repository_asks_its_hosting_service() {
    let (mut st, src, dest) = shallow_clone("size-service", 2, 1);
    git(&dest, &["remote", "set-url", "origin", "https://github.com/o/r.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = run(&mut st, full());
    match &fx[..] {
        [e @ Effect::Partial(PartialEffect::SizeHistory(r))] => {
            assert_eq!(r.url.as_deref(), Some("https://github.com/o/r.git"));
            let s = r.service.as_ref().expect("asks GitHub");
            assert_eq!(s.target.provider, Provider::GitHub);
            assert!(!s.connected);
            assert!(skipped(e), "tests never call the service");
        }
        _ => panic!("expected the size check"),
    }
    st.partial = Default::default();

    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: false }));
    assert!(!st.runs_git_unasked());
    match &run(&mut st, full())[..] {
        [Effect::Partial(PartialEffect::SizeHistory(r))] => assert!(r.service.is_none(), "signing in may run its helper"),
        _ => panic!("expected the size check"),
    }
    cleanup(&[&src, &dest]);
}
