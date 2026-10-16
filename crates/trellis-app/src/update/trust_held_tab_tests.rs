//! Repository trust: a job held in one tab is dropped cleanly when the prompt is answered while
//! another tab is active.

use super::rebase::JOB_LABEL;
use super::selection_tests::commit_on;
use super::tests::scratch_repo;
use super::trust_tests::{asking, drive, open, run, untrusted_repo};
use crate::msg::{Msg, RebaseMsg, TabsMsg, TrustMsg};
use crate::state::AppState;
use std::path::PathBuf;
use trellis_core::rebase::RebaseBase;

/// An untrusted repository with three commits after "first", its rebase panel open from the
/// first of them with the list reordered, Start clicked (so the rebase waits for the prompt),
/// and then a second repository opened in a new tab while the prompt is still up.
fn rebase_held_then_switched(name: &str) -> (AppState, PathBuf, PathBuf) {
    let a = untrusted_repo(&format!("{name}-a"));
    super::tests::git(&a, &["checkout", "--", "a.txt"]);
    let mut base = None;
    for i in 1..=3u32 {
        std::fs::write(a.join(format!("c{i}.txt")), format!("{i}\n")).unwrap();
        let oid = commit_on(&a, &format!("c{i}"), i + 1);
        base.get_or_insert(oid);
    }
    let mut st = super::tests::state();
    open(&mut st, &a);
    drive(&mut st, Msg::CloseModal);
    assert!(st.held());
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(base.unwrap()))));
    assert!(st.rebase.as_ref().is_some_and(|p| !p.loading && p.items.len() == 3), "the list loaded");
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    let fx = run(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false }));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.rebase.as_ref().unwrap().starting, "{:?}", st.log.last().map(|e| &e.text));
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some(JOB_LABEL));

    let b = scratch_repo(&format!("{name}-b"));
    open(&mut st, &b);
    assert_eq!(st.active_tab, 1);
    assert!(asking(&st), "A's prompt is still up");
    (st, a, b)
}

fn back_to_a(st: &mut AppState) {
    drive(st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let p = st.rebase.as_ref().expect("A's panel comes back");
    assert!(!p.starting, "the panel can be used again");
}

#[test]
fn cancel_from_another_tab_frees_the_held_rebase_panel() {
    let (mut st, a, b) = rebase_held_then_switched("held-tab-cancel");
    run(&mut st, Msg::CloseModal);
    assert!(st.trust.held.is_none() && st.job.is_none());
    assert!(st.log.last().is_some_and(|e| e.text.contains("only after you trust it")));
    back_to_a(&mut st);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn trust_and_run_from_another_tab_frees_the_panel_and_says_why() {
    let (mut st, a, b) = rebase_held_then_switched("held-tab-trust");
    let real_a = st.trust.held.as_ref().unwrap().real.clone();
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real: real_a, trusted: true }));
    assert!(!fx.iter().any(|e| matches!(e, crate::effect::Effect::InteractiveRebase { .. })), "{fx:?}");
    assert!(st.job.is_none());
    let text = &st.log.iter().rev().find(|e| e.text.contains("wasn't run")).expect("a log line").text;
    assert!(text.contains("trusted now") && !text.contains("only after you trust it"), "{text}");
    back_to_a(&mut st);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
