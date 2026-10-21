//! Headless tests for the Range-diff view: opening it (what it closes and keeps, the effect it
//! sends), the refusals (the same commit, an old git, a held repository), results and their
//! generations, the selection, Retry and Close, what closes it, the mainline it is cut at,
//! the entry points' reasons, parking it with its tab, and the whole round trip on a real
//! repository.

use super::tests::{clean_tree, drive, git, same_place, scratch_repo, state};
use super::trust_held_tests::held_repo;
use super::update;
use crate::effect::Effect;
use crate::journal::JournalMsg;
use crate::msg::{Msg, TrustMsg};
use crate::range_diff::{RangeDiffJob, RangeDiffMsg};
use crate::state::AppState;
use crate::state::blame::BlameView;
use crate::state::history::FileHistoryView;
use crate::state::journal::JournalView;
use crate::state::layout::{ToolView, tool_view};
use crate::state::range_diff::{self as rd, Phase};
use crate::state::rebase::RebasePanel;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::git_version::GitInfo;
use trellis_core::range_diff::{Compared, Ends, Outcome, Pair, Range, Status};
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{DiffTarget, FileDiff};

fn run(st: &mut AppState, m: RangeDiffMsg) -> Vec<Effect> {
    update(st, Msg::RangeDiff(m), Instant::now())
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

/// main with one commit; `topic` with two more, its tip amended; opened on main.
fn rewritten(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    write(&dir, "f.txt", "1\n2\n3\n4\n5\n6\n7\n8\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    write(&dir, "g.txt", "a\nb\nc\nd\ne\nf\ng\nh\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "two"]);
    write(&dir, "g.txt", "a\nb\nc\nD edited\ne\nf\ng\nh\n");
    git(&dir, &["commit", "-q", "-a", "--amend", "--no-edit"]);
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn tip(st: &AppState, branch: &str) -> Oid {
    st.snap.as_ref().unwrap().local_oid(branch).unwrap()
}

fn jobs(fx: &[Effect]) -> Vec<&RangeDiffJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::RangeDiff(j) => Some(j),
            _ => None,
        })
        .collect()
}

fn open_tips(st: &mut AppState) -> Vec<Effect> {
    let (old, new) = (tip(st, "main"), tip(st, "topic"));
    run(st, RangeDiffMsg::Open { old, new, label: None, at: None })
}

fn outcome(statuses: &[Status]) -> Box<Outcome> {
    let oid = Oid::from_bytes(&[1; 20]).unwrap();
    let pairs = statuses.iter().map(|s| Pair { status: *s, old: None, new: None, interdiff: None }).collect();
    let r = Range { base: oid, tip: oid };
    Box::new(Outcome::Compared(Compared { old: r, new: r, mainline: None, rewrite: None, pairs }))
}

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

#[test]
fn opening_asks_for_the_comparison_and_covers_the_graph() {
    let (mut st, dir) = rewritten("rd-open");
    let h = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), empty_diff().into()));
    st.history = Some(FileHistoryView::loading("a.txt".into(), None));
    st.blame = Some(BlameView::loading("a.txt".into(), None));
    st.rebase = Some(RebasePanel::loading(RebaseBase::FromCommit(h)));
    st.journal = Some(JournalView::default());
    let fx = open_tips(&mut st);
    let [job] = jobs(&fx)[..] else { panic!("one comparison: {fx:?}") };
    assert_eq!(job.ends, Ends::Tips { old: tip(&st, "main"), new: tip(&st, "topic") });
    assert_eq!(job.mainline.as_ref().map(|m| m.0.as_str()), Some("main"));
    let e = fx.iter().find(|e| matches!(e, Effect::RangeDiff(_))).unwrap();
    assert!(e.scope().is_some_and(|w| same_place(w, &dir)), "scoped to the repository");
    let view = st.range_diff.as_ref().expect("open");
    assert_eq!((view.phase.clone(), view.generation), (Phase::Loading, job.generation));
    assert!(st.diff.is_none() && st.history.is_none() && st.blame.is_none() && st.rebase.is_none());
    assert!(st.pull_request.is_none() && st.issue.is_none());
    assert!(st.journal.is_some(), "the journal stays underneath");
    assert_eq!(tool_view(&st), Some(ToolView::RangeDiff));
    assert!(!st.graph_shown());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals_before_anything_runs() {
    let (mut st, dir) = rewritten("rd-refuse");
    let main = tip(&st, "main");
    let fx = run(&mut st, RangeDiffMsg::Open { old: main, new: main, label: None, at: None });
    assert!(jobs(&fx).is_empty() && st.range_diff.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("The two versions are the same commit."));
    st.git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 18, 0)), chosen: false });
    for m in [
        RangeDiffMsg::Open { old: main, new: tip(&st, "topic"), label: None, at: None },
        RangeDiffMsg::OpenRewrite("refs/heads/topic".into()),
    ] {
        let fx = run(&mut st, m);
        assert!(jobs(&fx).is_empty() && st.range_diff.is_none());
        let text = &st.toasts.last().unwrap().text;
        assert_eq!(text, "Comparing two versions of a branch needs git 2.19 or newer (you have 2.18.0).");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_held_repository_waits_for_trust() {
    let (mut st, dir) = held_repo("rd-held");
    let fx = run(&mut st, RangeDiffMsg::OpenRewrite("refs/heads/main".into()));
    assert!(jobs(&fx).is_empty(), "nothing runs while held");
    assert_eq!(st.range_diff.as_ref().unwrap().phase, Phase::Held);
    assert!(matches!(st.modal, Some(crate::state::Modal::Trust(_))), "the trust prompt is asked");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let fx = update(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }), Instant::now());
    let [job] = jobs(&fx)[..] else { panic!("the comparison runs once trusted: {fx:?}") };
    assert_eq!(job.ends, Ends::LastRewrite { refname: "refs/heads/main".into() });
    assert_eq!(st.range_diff.as_ref().unwrap().phase, Phase::Loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn results_selection_retry_and_close() {
    let (mut st, dir) = rewritten("rd-results");
    let g = jobs(&open_tips(&mut st))[0].generation;
    run(&mut st, RangeDiffMsg::Loaded { generation: g - 1, result: Ok(outcome(&[Status::Changed])) });
    assert_eq!(st.range_diff.as_ref().unwrap().phase, Phase::Loading, "a stale result is ignored");
    run(&mut st, RangeDiffMsg::Loaded { generation: g, result: Ok(outcome(&[Status::Unchanged, Status::Changed, Status::Added])) });
    let v = st.range_diff.as_ref().unwrap();
    assert!(matches!(v.phase, Phase::Ready(_)));
    assert_eq!((v.selected, v.widest.len()), (Some(1), 3), "the first changed pair is selected");
    run(&mut st, RangeDiffMsg::Select(5));
    assert_eq!(st.range_diff.as_ref().unwrap().selected, Some(1), "out of range is ignored");
    run(&mut st, RangeDiffMsg::Select(2));
    assert_eq!(st.range_diff.as_ref().unwrap().selected, Some(2));
    let fx = run(&mut st, RangeDiffMsg::Retry);
    let [job] = jobs(&fx)[..] else { panic!("Retry asks again") };
    assert!(job.generation > g);
    assert_eq!(job.ends, Ends::Tips { old: tip(&st, "main"), new: tip(&st, "topic") });
    run(&mut st, RangeDiffMsg::Loaded { generation: job.generation, result: Ok(outcome(&[Status::Unchanged])) });
    assert_eq!(st.range_diff.as_ref().unwrap().selected, None, "no changed pair, nothing selected");
    let g = st.range_diff_gen;
    run(&mut st, RangeDiffMsg::Loaded { generation: g, result: Err("boom".into()) });
    assert_eq!(st.range_diff.as_ref().unwrap().phase, Phase::Failed("boom".into()));
    st.journal = Some(JournalView::default());
    run(&mut st, RangeDiffMsg::Close);
    assert!(st.range_diff.is_none() && st.journal.is_some());
    assert_eq!(tool_view(&st), Some(ToolView::Journal), "the journal shows again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_journal_and_the_git_flow_panel_close_it() {
    let (mut st, dir) = rewritten("rd-closed-by");
    open_tips(&mut st);
    update(&mut st, Msg::Journal(JournalMsg::Open), Instant::now());
    assert!(st.range_diff.is_none() && st.journal.is_some());
    open_tips(&mut st);
    update(&mut st, Msg::GitFlow(crate::gitflow::GitFlowMsg::OpenPanel), Instant::now());
    assert!(st.range_diff.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_mainline_leaves_out_what_is_compared() {
    let (mut st, dir) = rewritten("rd-mainline");
    git(&dir, &["branch", "-q", "--set-upstream-to", "main", "topic"]);
    git(&dir, &["branch", "-q", "master", "main"]);
    drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Open(dir.clone())));
    let fx = run(&mut st, RangeDiffMsg::OpenRewrite("refs/heads/topic".into()));
    let [job] = jobs(&fx)[..] else { panic!("one comparison") };
    assert_eq!(job.ends, Ends::LastRewrite { refname: "refs/heads/topic".into() });
    assert_eq!(job.mainline.as_ref().map(|m| m.0.as_str()), Some("master"), "not topic's upstream, main");
    let (topic, main) = (tip(&st, "topic"), tip(&st, "main"));
    let remote = st.snap.as_mut().unwrap();
    remote.remotes = vec![("origin".into(), vec![("x".into(), topic), ("main".into(), main)])];
    remote.locals.iter_mut().filter(|b| b.name == "topic").for_each(|b| {
        b.tracking = Some(trellis_core::repo::Tracking { remote: "origin".into(), merge: "x".into() });
    });
    let remote = st.snap.as_ref().unwrap();
    let (r, exclude) = rd::exclude_for_remote(remote, "origin", "x");
    assert_eq!(r.as_deref(), Some("origin"));
    assert_eq!(exclude, ["origin/x", "topic"]);
    let origin =
        rd::Origin { ends: Ends::LastRewrite { refname: "refs/remotes/origin/x".into() }, label: Some("origin/x".into()), at: None };
    assert_eq!(rd::mainline_for(remote, &origin).map(|m| m.0).as_deref(), Some("origin/main"));
    let _ = std::fs::remove_dir_all(&dir);
}
