//! Headless tests for the pull request panel's code comment threads: replying, resolving,
//! which threads are shown and open, and going from a thread to its line.

use super::super::tests::finish_keeping;
use super::super::update;
use super::calls_tests::{calls, detail, done, err, opened, reply_msg, run, view};
use super::tests::{github, pr_target};
use crate::effect::{Effect, ForgeError};
use crate::msg::Msg;
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction, PrTab};
use glowfly_core::diff_rows::{LineSide, row_of_file_line};
use glowfly_core::forge::{Auth, PrPage, Provider, ReviewThread, ThreadComment};
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

fn comment(id: u64, time: i64) -> ThreadComment {
    ThreadComment { id: Some(id), author: "ada".into(), time, body: "Rename this".into(), url: String::new(), pending: false }
}

/// A current thread on `path` at new line `line`, first comment 55.
fn thread(id: &str, path: &str, line: Option<u32>) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: path.into(),
        side: LineSide::New,
        line,
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: String::new(),
        comments: vec![comment(55, 1)],
        more: 0,
    }
}

/// The opened panel with a page of `threads` loaded (head = the local source tip).
fn loaded_threads(st: &mut AppState, threads: Vec<ReviewThread>, f: impl FnOnce(&mut glowfly_core::forge::PrDetail)) {
    let head = st.forge.as_ref().unwrap().prs[0].head.clone().unwrap_or_default();
    let mut d = detail(&head);
    f(&mut d);
    let page = PrPage { detail: d, entries: Vec::new(), threads, threads_note: None };
    let msg = reply_msg(st, PrReply::Loaded(Ok(page)));
    run(st, msg);
}

fn draft(st: &mut AppState, id: &str, text: &str) {
    st.pull_request.as_mut().unwrap().forms.replies.insert(id.into(), text.into());
}

fn the_thread<'a>(st: &'a AppState, id: &str) -> &'a ReviewThread {
    view(st).loaded().unwrap().threads.iter().find(|t| t.id == id).unwrap()
}

#[test]
fn reply_sends_the_threads_first_comment() {
    let (mut st, _, dirs) = opened("prt-reply");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    draft(&mut st, "T1", "  Done  \n");
    let fx = run(&mut st, PullRequestMsg::Reply("T1".into()));
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    assert!(matches!(&job.call, PrCall::Reply { thread, comment: 55, body } if thread == "T1" && body == "Done"), "{:?}", job.call);
    assert_eq!(view(&st).busy, Some(PrAction::Reply("T1".into())));
    assert!(calls(&run(&mut st, PullRequestMsg::Reply("T1".into()))).is_empty(), "busy");
    assert!(calls(&run(&mut st, PullRequestMsg::Resolve { thread: "T1".into(), resolve: true })).is_empty(), "busy");
    done(st, dirs);
}

#[test]
fn reply_blocked_by_pending_review_or_empty_sends_nothing() {
    let (mut st, _, dirs) = opened("prt-reply-blocked");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |d| d.pending_review = Some(9));
    draft(&mut st, "T1", "Done");
    assert!(calls(&run(&mut st, PullRequestMsg::Reply("T1".into()))).is_empty(), "a review in progress");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    draft(&mut st, "T1", "   ");
    assert!(calls(&run(&mut st, PullRequestMsg::Reply("T1".into()))).is_empty(), "empty");
    assert!(calls(&run(&mut st, PullRequestMsg::Reply("T9".into()))).is_empty(), "no such thread");
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}

#[test]
fn replied_ok_appends_comment_clears_draft_toasts_and_logs() {
    let (mut st, _, dirs) = opened("prt-replied");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    draft(&mut st, "T1", "Done");
    run(&mut st, PullRequestMsg::Reply("T1".into()));
    let reply = comment(77, 9);
    let msg = reply_msg(&st, PrReply::RepliedToThread { thread: "T1".into(), result: Ok(reply.clone()) });
    let fx = run(&mut st, msg);
    assert!(calls(&fx).is_empty(), "the page isn't loaded again");
    let v = view(&st);
    assert_eq!(v.busy, None);
    assert!(!v.forms.replies.contains_key("T1"));
    assert_eq!(the_thread(&st, "T1").comments.last(), Some(&reply));
    assert!(st.toasts.iter().any(|t| t.text == "Reply posted"));
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Replied on #7"));
    done(st, dirs);
}

#[test]
fn replied_err_keeps_draft_sets_error_for_reply_and_logs() {
    let (mut st, _, dirs) = opened("prt-reply-err");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    draft(&mut st, "T1", "Done");
    run(&mut st, PullRequestMsg::Reply("T1".into()));
    let msg = reply_msg(&st, PrReply::RepliedToThread { thread: "T1".into(), result: Err(err("HTTP 422: nope")) });
    run(&mut st, msg);
    let v = view(&st);
    assert_eq!(v.forms.replies["T1"], "Done");
    assert_eq!(v.action_error, Some((PrAction::Reply("T1".into()), "HTTP 422: nope".into())));
    assert_eq!(the_thread(&st, "T1").comments.len(), 1);
    let last = st.log.last().unwrap();
    assert_eq!((last.ok, last.text.as_str()), (false, "Replying on #7 failed: HTTP 422: nope"));
    done(st, dirs);
}

#[test]
fn resolve_sends_call_and_reply_flips_state_and_collapses() {
    let (mut st, _, dirs) = opened("prt-resolve");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    st.pull_request.as_mut().unwrap().threads.open.insert("T1".into(), true);
    let fx = run(&mut st, PullRequestMsg::Resolve { thread: "T1".into(), resolve: true });
    assert!(matches!(&calls(&fx)[..], [j] if matches!(&j.call, PrCall::Resolve { thread, resolve: true } if thread == "T1")));
    assert_eq!(view(&st).busy, Some(PrAction::Resolve("T1".into())));
    let msg = reply_msg(&st, PrReply::Resolved { thread: "T1".into(), resolve: true, result: Ok(true) });
    assert!(calls(&run(&mut st, msg)).is_empty());
    let t = the_thread(&st, "T1");
    assert!(t.resolved && t.can_unresolve);
    assert!(!view(&st).threads.open.contains_key("T1"), "its Show/Hide is dropped, so it collapses");
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Resolved a thread on #7"));
    assert!(st.toasts.iter().any(|t| t.text == "Resolved a thread on #7"));

    let mut t = the_thread(&st, "T1").clone();
    t.can_resolve = false;
    loaded_threads(&mut st, vec![t], |_| {});
    run(&mut st, PullRequestMsg::Resolve { thread: "T1".into(), resolve: false });
    let msg = reply_msg(&st, PrReply::Resolved { thread: "T1".into(), resolve: false, result: Ok(false) });
    run(&mut st, msg);
    let t = the_thread(&st, "T1");
    assert!(!t.resolved && t.can_resolve);
    assert!(st.log.last().is_some_and(|l| l.text == "Reopened a thread on #7"));
    done(st, dirs);
}

#[test]
fn resolve_auth_error_drops_cached_auth() {
    let (mut st, _, dirs) = opened("prt-auth");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    let key = github().account().key();
    st.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    run(&mut st, PullRequestMsg::Resolve { thread: "T1".into(), resolve: true });
    let failed = ForgeError { message: "authentication failed: 401".into(), auth: true };
    let msg = reply_msg(&st, PrReply::Resolved { thread: "T1".into(), resolve: true, result: Err(failed) });
    run(&mut st, msg);
    assert!(!st.forge.as_ref().unwrap().auth.contains_key(&key));
    assert!(st.log.last().is_some_and(|l| l.text == "Resolving a thread on #7 failed: authentication failed: 401"));
    assert_eq!(view(&st).action_error.as_ref().map(|(a, _)| a.clone()), Some(PrAction::Resolve("T1".into())));
    done(st, dirs);
}

#[test]
fn thread_reply_for_other_workdir_is_only_logged() {
    let (mut st, _, dirs) = opened("prt-elsewhere");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    let before = view(&st).loaded().cloned();
    let mut msg = reply_msg(&st, PrReply::RepliedToThread { thread: "T1".into(), result: Ok(comment(77, 9)) });
    if let PullRequestMsg::Replied { workdir, .. } = &mut msg {
        *workdir = glowfly_core::paths::temp_dir().join("glowfly-elsewhere");
    }
    run(&mut st, msg);
    assert_eq!(view(&st).loaded().cloned(), before, "the page is unchanged");
    assert!(st.toasts.iter().all(|t| t.text != "Reply posted"));
    assert!(st.log.last().is_some_and(|l| l.text == "Replied on #7"));
    assert!(!st.view.show_log, "quietly");
    done(st, dirs);
}

#[test]
fn show_resolved_outdated_and_toggle_thread_update_view() {
    let (mut st, _, dirs) = opened("prt-show");
    let mut done_thread = thread("T2", "f.txt", Some(1));
    done_thread.resolved = true;
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1)), done_thread], |_| {});
    run(&mut st, PullRequestMsg::ShowResolved(true));
    run(&mut st, PullRequestMsg::ShowOutdated(true));
    let tv = &view(&st).threads;
    assert!(tv.show_resolved && tv.show_outdated);
    run(&mut st, PullRequestMsg::ToggleThread("T1".into()));
    run(&mut st, PullRequestMsg::ToggleThread("T2".into()));
    let tv = &view(&st).threads;
    assert_eq!((tv.open.get("T1"), tv.open.get("T2")), (Some(&false), Some(&true)), "each flips from its default");
    run(&mut st, PullRequestMsg::ToggleThread("T1".into()));
    assert_eq!(view(&st).threads.open.get("T1"), Some(&true));
    run(&mut st, PullRequestMsg::ShowResolved(false));
    assert!(!view(&st).threads.show_resolved);
    done(st, dirs);
}

#[test]
fn focus_thread_sets_focus_open_and_scroll_request() {
    let (mut st, _, dirs) = opened("prt-focus");
    let mut done_thread = thread("T1", "f.txt", Some(1));
    done_thread.resolved = true;
    loaded_threads(&mut st, vec![done_thread], |_| {});
    run(&mut st, PullRequestMsg::FocusThread("T1".into()));
    let tv = &view(&st).threads;
    assert_eq!((tv.focus.as_deref(), tv.open.get("T1")), (Some("T1"), Some(&true)));
    assert_eq!(st.view.pr_thread_scroll.as_deref(), Some("T1"));
    run(&mut st, PullRequestMsg::FocusThread("T9".into()));
    assert_eq!(view(&st).threads.focus.as_deref(), Some("T1"), "no such thread");
    done(st, dirs);
}

#[test]
fn go_to_thread_opens_file_and_lands_on_line() {
    let (mut st, _, dirs) = opened("prt-goto");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    let target = pr_target(&st, "f.txt");
    let DiffTarget::Between(base, tip, _) = target.clone() else { panic!("a comparison") };
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::GoToThread("T1".into())), Instant::now());
    let loads: Vec<&DiffTarget> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::LoadDiff { target, .. } => Some(target),
            _ => None,
        })
        .collect();
    assert_eq!(loads, [&DiffTarget::Between(base, tip, "f.txt".into())]);
    assert_eq!(view(&st).tab, PrTab::Files);
    assert_eq!(st.view.diff_open_line, Some((target.clone(), LineSide::New, 1)));
    assert_eq!(view(&st).threads.focus.as_deref(), Some("T1"));
    finish_keeping(&mut st, fx);
    let (open, d) = st.diff.as_ref().expect("the diff loaded");
    assert_eq!(*open, target);
    let want = row_of_file_line(d, st.settings.diff_mode, st.settings.diff_layout, &st.view.diff_expanded, LineSide::New, 1);
    assert!(want.is_some());
    assert_eq!(st.view.diff_scroll_to_row, want);
    assert_eq!(st.view.diff_open_line, None, "taken");
    done(st, dirs);
}

#[test]
fn go_to_thread_with_diff_open_scrolls_without_loading() {
    let (mut st, _, dirs) = opened("prt-goto-open");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1)), thread("T2", "f.txt", Some(40))], |_| {});
    let target = pr_target(&st, "f.txt");
    let fx = update(&mut st, Msg::Repo(crate::msg::RepoMsg::OpenDiff(target)), Instant::now());
    finish_keeping(&mut st, fx);
    run(&mut st, PullRequestMsg::Tab(PrTab::Overview));
    st.view.diff_scroll_to_row = None;
    let fx = run(&mut st, PullRequestMsg::GoToThread("T1".into()));
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadDiff { .. })));
    assert_eq!(view(&st).tab, PrTab::Files);
    assert_eq!(st.view.diff_scroll_to_row, Some(1), "under the hunk header");
    assert_eq!(st.view.pr_thread_scroll.as_deref(), Some("T1"));
    // A line outside the hunks: nothing scrolls, and the reason is logged quietly.
    st.view.diff_scroll_to_row = None;
    run(&mut st, PullRequestMsg::GoToThread("T2".into()));
    assert_eq!(st.view.diff_scroll_to_row, None);
    assert!(st.log.last().is_some_and(|l| l.text.starts_with("Line 40 isn't in the changed lines")));
    done(st, dirs);
}

#[test]
fn go_to_outdated_or_unknown_path_does_nothing() {
    let (mut st, _, dirs) = opened("prt-goto-none");
    let mut old = thread("T1", "f.txt", None);
    old.outdated = true;
    old.original_line = Some(1);
    loaded_threads(&mut st, vec![old, thread("T2", "F.TXT", Some(1)), thread("T3", "../f.txt", Some(1))], |_| {});
    for id in ["T1", "T2", "T3"] {
        let fx = run(&mut st, PullRequestMsg::GoToThread(id.into()));
        assert!(fx.is_empty(), "{id}");
        assert!(st.diff.is_none(), "{id}");
        assert_eq!(view(&st).tab, PrTab::Overview, "{id}");
        assert_eq!(st.view.diff_open_line, None, "{id}");
    }
    done(st, dirs);
}

#[test]
fn reload_keeps_drafts_and_clears_focus_of_vanished_thread() {
    let (mut st, _, dirs) = opened("prt-reload");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    run(&mut st, PullRequestMsg::FocusThread("T1".into()));
    draft(&mut st, "T1", "Half a thought");
    loaded_threads(&mut st, vec![thread("T1", "f.txt", Some(1))], |_| {});
    assert_eq!(view(&st).threads.focus.as_deref(), Some("T1"), "still there");
    loaded_threads(&mut st, vec![thread("T2", "f.txt", Some(1))], |_| {});
    let v = view(&st);
    assert_eq!(v.threads.focus, None);
    assert_eq!(v.forms.replies["T1"], "Half a thought");
    assert!(matches!(&v.page, Page::Loaded(p) if p.threads[0].id == "T2"));
    done(st, dirs);
}

#[test]
fn thread_intents_keep_nothing_for_inactive_tabs() {
    let msgs = [
        PullRequestMsg::Reply("T".into()),
        PullRequestMsg::Resolve { thread: "T".into(), resolve: true },
        PullRequestMsg::ShowResolved(true),
        PullRequestMsg::ShowOutdated(true),
        PullRequestMsg::ToggleThread("T".into()),
        PullRequestMsg::FocusThread("T".into()),
        PullRequestMsg::GoToThread("T".into()),
    ];
    for m in msgs {
        assert!(m.clone().on_inactive().is_none(), "{m:?}");
    }
}
