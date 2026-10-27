//! Headless tests for cherry-picking several selected commits: the command, refusals, and
//! end to end against real repositories, including a conflict part way.

use super::selection_tests::{commit_on, fixture, git, refresh, up};
use super::tests::{drive, state};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::selection::CommitBlock;
use crate::state::{AppState, RunningJob, Selection};
use git2::{Oid, RepositoryState};
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use std::path::PathBuf;

#[test]
fn cherry_pick_orders_oldest_first() {
    let (mut st, f) = fixture("multi-pick-order");
    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(1).to_string()]);
    refresh(&mut st);
    let fx = up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f.c(4), f.c(2), f.c(3)])));
    let expected = args(&["cherry-pick", &f.c(2).to_string(), &f.c(3).to_string(), &f.c(4).to_string()]);
    assert!(matches!(&fx[..], [Effect::Pick(j)] if j.cmds == vec![expected] && j.record.is_some() && j.can_skip), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Cherry-pick 3 commits"));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn cherry_pick_refusals() {
    let (mut st, f) = fixture("multi-pick-refused");
    // Commits already on the current branch.
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f.c(4), f.c(5)]))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(CommitBlock::PickOnHead.text(Lang::En)));
    // In German, the log says so in German.
    st.settings.language = "de".into();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f.c(4), f.c(5)]))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(CommitBlock::PickOnHead.text(Lang::De)));
    assert_ne!(CommitBlock::PickOnHead.text(Lang::De), CommitBlock::PickOnHead.text(Lang::En));
    st.settings.language = "en".into();
    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(1).to_string()]);
    refresh(&mut st);
    // Another job is running.
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: Some(f.dir.clone()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f.c(3), f.c(2)]))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Another operation is still running."));
    let _ = std::fs::remove_dir_all(&f.dir);
}

/// A repository on `main` with `feature` holding `commits` (message, contents of f.txt).
pub(super) fn feature_repo(name: &str, feature: &[(&str, &str)], main_edit: Option<&str>) -> (AppState, PathBuf, Vec<Oid>) {
    let dir = super::tests::scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("f.txt"), "base\n").unwrap();
    commit_on(&dir, "base", 1);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    let picks = feature
        .iter()
        .enumerate()
        .map(|(i, (msg, text))| {
            std::fs::write(dir.join("f.txt"), text).unwrap();
            commit_on(&dir, msg, 2 + i as u32)
        })
        .collect();
    git(&dir, &["checkout", "-q", "main"]);
    if let Some(text) = main_edit {
        std::fs::write(dir.join("f.txt"), text).unwrap();
        commit_on(&dir, "main edit", 9);
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, picks)
}

#[test]
fn cherry_pick_end_to_end() {
    let (mut st, dir, f) = feature_repo("multi-pick-e2e", &[("f1", "base\n1\n"), ("f2", "base\n1\n2\n"), ("f3", "base\n1\n2\n3\n")], None);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f[2], f[0], f[1]])));
    assert!(st.job.is_none());
    assert_eq!(git(&dir, &["log", "--format=%s", "-4", "main"]).trim().lines().collect::<Vec<_>>(), ["f3", "f2", "f1", "base"]);
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n1\n2\n3\n");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Cherry-pick 3 commits succeeded"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_pick_conflict_shows_banner_state() {
    let (mut st, dir, f) = feature_repo("multi-pick-conflict", &[("x1", "theirs\n"), ("x2", "theirs\nmore\n")], Some("ours\n"));
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f[0], f[1]])));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Stopped on conflicts in 1 file. Resolve it, then continue or abort."));
    let state = st.snap.as_ref().unwrap().state;
    assert!(matches!(state, RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{state:?}");
    let actions = crate::ui::common::state_actions(state).unwrap();
    let title = crate::state::op_actions::banner_title(glowfly_core::i18n::Lang::En, st.snap.as_ref().unwrap());
    assert_eq!(title.as_deref(), Some("Cherry-pick in progress"));
    let labels: Vec<&str> = actions.iter().map(|(l, _)| *l).collect();
    assert_eq!(labels, ["Continue", "Skip commit", "Abort cherry-pick"]);
    assert_eq!(st.session.selection, Selection::Wip, "the WIP row shows the banner");
    let (label, abort) = actions[2];
    drive(&mut st, Msg::git(label, abort));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}
