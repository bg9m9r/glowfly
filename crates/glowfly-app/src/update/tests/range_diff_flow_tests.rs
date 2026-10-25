//! The Range-diff view end to end and across tabs: the real runner on a rewritten branch and on
//! one never rewritten, parking with a comparison in flight, and why the journal's and the
//! multi-commit menu's entry points are off.

use super::tests::{clean_tree, drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::range_diff::RangeDiffMsg;
use crate::state::AppState;
use crate::state::Selection;
use crate::state::range_diff::{self as rd, Phase};
use crate::state::selection::MultiSelection;
use glowfly_core::git_version::GitInfo;
use glowfly_core::i18n::Lang;
use glowfly_core::journal::{Event, Row, Subject};
use glowfly_core::range_diff::{Outcome, Status};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn commit(dir: &Path, file: &str, text: &str, msg: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

/// main, and `topic` with two commits whose tip was amended (or not, without `amend`); opened.
fn repo(name: &str, amend: bool) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    commit(&dir, "f.txt", "1\n2\n3\n4\n5\n6\n7\n8\n", "one");
    commit(&dir, "g.txt", "a\nb\nc\nd\ne\nf\ng\nh\n", "two");
    if amend {
        std::fs::write(dir.join("g.txt"), "a\nb\nc\nD edited\ne\nf\ng\nh\n").unwrap();
        git(&dir, &["commit", "-q", "-a", "--amend", "--no-edit"]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn outcome(st: &AppState) -> &Outcome {
    match &st.range_diff.as_ref().expect("open").phase {
        Phase::Ready(o) => o,
        other => panic!("not ready: {other:?}"),
    }
}

#[test]
fn a_rewritten_branch_end_to_end() {
    let (mut st, dir) = repo("rd-e2e", true);
    drive(&mut st, Msg::RangeDiff(RangeDiffMsg::OpenRewrite("refs/heads/topic".into())));
    let Outcome::Compared(c) = outcome(&st) else { panic!("{:?}", outcome(&st)) };
    let statuses: Vec<Status> = c.pairs.iter().map(|p| p.status).collect();
    assert_eq!(statuses, [Status::Unchanged, Status::Changed]);
    assert_eq!(c.mainline.as_deref(), Some("main"));
    let v = st.range_diff.as_ref().unwrap();
    assert_eq!(v.selected, Some(1));
    assert!(v.widest[1] > 0, "the interdiff's width is measured");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_never_rewritten_says_so() {
    let (mut st, dir) = repo("rd-e2e-none", false);
    drive(&mut st, Msg::RangeDiff(RangeDiffMsg::OpenRewrite("refs/heads/topic".into())));
    let o = outcome(&st);
    assert_eq!(*o, Outcome::NoRewrite { refname: "refs/heads/topic".into() });
    let text = rd::outcome_failure(Lang::En, o).unwrap();
    assert!(text.starts_with("There is no earlier version of topic to compare with"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn it_is_parked_with_its_tab_and_asked_for_again_on_return() {
    let (mut st, dir) = repo("rd-tabs", true);
    update(&mut st, Msg::RangeDiff(RangeDiffMsg::OpenRewrite("refs/heads/topic".into())), Instant::now());
    let before = st.range_diff_gen;
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(st.range_diff.is_none(), "the new tab has none");
    let workdir = st.tabs[0].path.clone().unwrap();
    let loaded = || RangeDiffMsg::Loaded { generation: before, result: Err("late".into()) };
    assert!(Msg::RangeDiff(loaded()).on_inactive().is_none(), "nothing is kept for an inactive tab");
    let late = Msg::RangeDiff(loaded());
    update(&mut st, Msg::Scoped { workdir, msg: Box::new(late) }, Instant::now());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let v = st.range_diff.as_ref().expect("back with its tab");
    assert_eq!(v.phase, Phase::Loading, "the late result was dropped");
    let again: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::RangeDiff(j) => Some(j.generation),
            _ => None,
        })
        .collect();
    assert_eq!(again.len(), 1, "the comparison in flight is asked for again");
    assert!(again[0] > before && v.generation == again[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn row(subject: Subject, oid: git2::Oid, summary: Option<&str>) -> Row {
    Row {
        subject,
        oid,
        prev: None,
        at: 0,
        event: Event::Other,
        message: String::new(),
        note: None,
        summary: summary.map(str::to_owned),
        current: false,
        unreachable: false,
    }
}

#[test]
fn why_the_entry_points_are_off() {
    let (mut st, dir) = repo("rd-blocked", true);
    let t = Lang::En;
    let snap = st.snap.as_ref().unwrap();
    let (main, topic) = (snap.local_oid("main").unwrap(), snap.local_oid("topic").unwrap());
    let branch = |b: &str| Subject::Branch(b.into());
    let blocked = |st: &AppState, r: &Row| rd::journal_blocked(t, st.snap.as_ref().unwrap(), r, &st.git);
    assert_eq!(blocked(&st, &row(branch("topic"), main, None)).as_deref(), Some("This commit is no longer in the repository."));
    assert_eq!(blocked(&st, &row(branch("gone"), main, Some("x"))).as_deref(), Some("gone no longer exists."));
    assert_eq!(blocked(&st, &row(branch("topic"), topic, Some("x"))).as_deref(), Some("topic still points at this commit."));
    assert_eq!(blocked(&st, &row(branch("topic"), main, Some("x"))), None);
    assert_eq!(blocked(&st, &row(Subject::Head, topic, Some("x"))), None, "HEAD is on main");
    st.git.info = Some(GitInfo { path: None, version: Some((2, 18, 0)), chosen: false });
    let old = blocked(&st, &row(branch("topic"), main, Some("x"))).unwrap();
    assert!(old.contains("2.19"), "{old}");

    let snap = st.snap.as_ref().unwrap();
    let Selection::Commits(two) = MultiSelection::build([main, topic], main, snap) else { panic!("two commits") };
    assert_eq!(rd::multi_blocked(t, &two), None);
    assert_eq!(two.oids, [topic, main], "graph order, newest first: oids[1] is the old version");
    let one = snap.commits.iter().find(|c| c.summary == "one").map(|c| c.oid).unwrap();
    let Selection::Commits(three) = MultiSelection::build([main, one, topic], main, snap) else { panic!("three commits") };
    assert_eq!(rd::multi_blocked(t, &three), Some("Select exactly two commits."));
    let _ = std::fs::remove_dir_all(&dir);
}
