//! The diff's card for a submodule's gitlink: a move forward (with Open submodule and Record
//! this checkout), back, sideways from a common commit and to an unrelated history, the same
//! commit with changes inside, commits the submodule lacks, a long list cut to its newest
//! commits, and a submodule added. The move is injected beside a "Subproject commit" diff.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_submodules::pushed;
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{
    CommitRun, DiffLine, DiffTarget, FileDiff, GITLINK_LIST_MAX, GitlinkMove, GitlinkUnknown, Hunk, MoveKind, SubmoduleInfo, SubmoduleState,
};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::submodule::SubmoduleMsg;
use std::path::PathBuf;

const PATH: &str = "libs/lib";

fn oid(c: char) -> Oid {
    Oid::from_str(&c.to_string().repeat(40)).unwrap()
}

/// `subjects` as commits with made-up ids, oldest first; `total` counted.
fn run(subjects: &[&str], total: usize) -> CommitRun {
    let commits = subjects
        .iter()
        .enumerate()
        .map(|(i, s)| (Oid::from_str(&format!("{:07x}{}", 0x3f2a1c0 + i * 0x1d3b, "0".repeat(33))).unwrap(), s.to_string()))
        .collect();
    CommitRun { commits, total, capped: false }
}

fn moved(kind: MoveKind) -> GitlinkMove {
    GitlinkMove {
        path: PATH.into(),
        old: Some(oid('a')),
        new: Some(oid('b')),
        kind,
        ahead: CommitRun::default(),
        behind: CommitRun::default(),
        dirty: false,
        type_change: false,
    }
}

fn subproject_rows() -> FileDiff {
    let line = |origin: char, c: char| DiffLine { origin, old: Some(1), new: Some(1), text: format!("Subproject commit {}", oid(c)) };
    FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1 +1 @@".into(), lines: vec![line('-', 'a'), line('+', 'b')] }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
    }
}

/// The fixture with the submodule's row (`state`) and the diff of `target` showing `m`.
fn card(theme: ThemeChoice, name: &str, target: DiffTarget, state: SubmoduleState, m: GitlinkMove) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.snap.as_mut().unwrap().submodules = vec![SubmoduleInfo {
        name: PATH.into(),
        path: PATH.into(),
        url: Some("../lib.git".into()),
        clone_url: None,
        branch: None,
        added: false,
        recorded: oid('a'),
        checked_out: Some(oid('b')),
        state,
        left_behind: Vec::new(),
        outside_sparse: false,
    }];
    st.session.diff.view = Some((target.clone(), subproject_rows().into()));
    st.session.diff.gitlink = Some((target, Box::new(m)));
    (st, dir)
}

fn commit(st: &AppState) -> DiffTarget {
    DiffTarget::Commit(st.snap.as_ref().and_then(|s| s.head_oid).unwrap(), PATH.into())
}

/// Draws `st` and checks it as `name`.
fn check(st: AppState, dark: bool, name: &str) -> egui_kittest::Harness<'static, (AppState, Vec<Msg>)> {
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name);
    h
}

#[test]
fn gitlink_card_forward() {
    let m = GitlinkMove { ahead: run(&["Parse config lazily", "Fix the reader's buffer", "Release 2.1"], 3), ..moved(MoveKind::Forward) };
    let target = DiffTarget::Unstaged(PATH.into());
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-forward", target, SubmoduleState::OutOfSync, m);
    let mut h = check(st, true, "gitlink_card_forward");
    assert!(h.query_by_label("Submodule moved forward 3 commits").is_some());
    assert!(h.query_by_label_contains("Subproject commit").is_none(), "the card instead of the rows");
    h.get_by_label("Open submodule").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Open(p)) if p == PATH)));
    h.get_by_label("Record this checkout").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Record(p)) if p == PATH)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_backward() {
    let m = GitlinkMove { behind: run(&["Add a cache", "Use the cache"], 2), ..moved(MoveKind::Backward) };
    let (st, dir) = with_repo(ThemeChoice::Light, "gitlink-backward");
    let target = commit(&st);
    drop(st);
    let (st, _) = card(ThemeChoice::Light, "gitlink-backward", target, SubmoduleState::InSync, m);
    let h = check(st, false, "gitlink_card_backward");
    assert!(h.query_by_label("Submodule moved back 2 commits").is_some());
    assert!(h.query_by_label("Record this checkout").is_none(), "only an unstaged diff records");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_sideways() {
    let base = Some((oid('c'), "Split the parser".to_owned()));
    let m = GitlinkMove {
        ahead: run(&["Try a new lexer"], 1),
        behind: run(&["Tune the old lexer", "Document the lexer"], 2),
        ..moved(MoveKind::Sideways { base })
    };
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-sideways", DiffTarget::Staged(PATH.into()), SubmoduleState::InSync, m);
    let h = check(st, true, "gitlink_card_sideways");
    assert!(h.query_by_label("Both start from ccccccc Split the parser").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_unrelated() {
    let m = GitlinkMove { ahead: run(&["Start over"], 1), behind: run(&["Initial commit"], 1), ..moved(MoveKind::Sideways { base: None }) };
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-unrelated", DiffTarget::Staged(PATH.into()), SubmoduleState::InSync, m);
    let h = check(st, true, "gitlink_card_unrelated");
    assert!(h.query_by_label("Submodule moved to a commit with unrelated history").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_same_dirty() {
    let m = GitlinkMove { new: Some(oid('a')), dirty: true, ..moved(MoveKind::Same) };
    let target = DiffTarget::Unstaged(PATH.into());
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-same-dirty", target, SubmoduleState::Dirty, m);
    let h = check(st, true, "gitlink_card_same_dirty");
    assert!(h.query_by_label_contains("It also has uncommitted changes inside").is_some());
    assert!(h.query_by_label("Record this checkout").is_none(), "nothing to record: the same commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_missing() {
    let m = moved(MoveKind::Unknown(GitlinkUnknown::Missing { old: false, new: true }));
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-missing", DiffTarget::Staged(PATH.into()), SubmoduleState::InSync, m);
    let h = check(st, true, "gitlink_card_missing");
    assert!(h.query_by_label_contains("Fetch in the submodule to list the commits.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_capped() {
    let subjects: Vec<String> = (11..=60).map(|i| format!("Step {i}")).collect();
    let subjects: Vec<&str> = subjects.iter().map(String::as_str).collect();
    let m = GitlinkMove { ahead: run(&subjects, 60), ..moved(MoveKind::Forward) };
    assert_eq!(m.ahead.commits.len(), GITLINK_LIST_MAX);
    let (st, dir) = card(ThemeChoice::Dark, "gitlink-capped", DiffTarget::Staged(PATH.into()), SubmoduleState::InSync, m);
    let h = check(st, true, "gitlink_card_capped");
    assert!(h.query_by_label("10 earlier commits not shown").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitlink_card_added() {
    let m = GitlinkMove { old: None, ..moved(MoveKind::Added) };
    let (st, dir) = with_repo(ThemeChoice::Dark, "gitlink-added");
    let target = commit(&st);
    drop(st);
    let (st, _) = card(ThemeChoice::Dark, "gitlink-added", target, SubmoduleState::InSync, m);
    let h = check(st, true, "gitlink_card_added");
    assert!(h.query_by_label("Submodule added at bbbbbbb").is_some());
    drop(h);
    remove_fixture(&dir);
}
