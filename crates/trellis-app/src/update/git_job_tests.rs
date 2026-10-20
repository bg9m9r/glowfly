//! Which jobs carry a journal note: worked out from the command as given, before the
//! auto-stash wrap and the hard reset's own stash rewrite it, and never for another repository.

use super::jobs::{JobSpec, start_job};
use super::journal_tests::opened;
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;
use trellis_core::journal::Op;

/// The journal op of the job `cmd` starts as a `JobMsg::Git`.
fn op_of(st: &mut AppState, cmd: &[&str]) -> Option<Op> {
    let fx = update(st, Msg::git("Job", cmd), Instant::now());
    let op = match fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) {
        Some(Effect::RunGit { journal, .. }) => journal.clone(),
        _ => panic!("{cmd:?} starts a git job"),
    };
    st.job = None;
    op
}

#[test]
fn history_rewrites_carry_a_note_and_everyday_commands_do_not() {
    let (mut st, dir) = opened("journal-git-job");
    let first = trellis_core::gitcli::read(&dir, &trellis_core::gitcli::args(&["rev-parse", "topic"])).stdout.trim().to_owned();
    assert!(matches!(op_of(&mut st, &["rebase", "topic"]), Some(Op::Rebase { .. })));
    assert!(matches!(op_of(&mut st, &["merge", "topic"]), Some(Op::Merge { .. })));
    assert!(matches!(op_of(&mut st, &["pull"]), Some(Op::Pull)));
    assert!(matches!(op_of(&mut st, &["cherry-pick", &first]), Some(Op::CherryPick { .. })));
    assert!(matches!(op_of(&mut st, &["revert", &first]), Some(Op::Revert { .. })));
    // The hard reset runs as its own stash and then the reset, still noted as the reset.
    assert_eq!(op_of(&mut st, &["reset", "--hard", &first]), Some(Op::Reset { mode: "hard".into(), to: first[..7].into() }));
    assert!(matches!(op_of(&mut st, &["reset", "--soft", &first]), Some(Op::Reset { .. })));
    assert_eq!(op_of(&mut st, &["commit", "--amend", "-m", "x"]), Some(Op::Amend));
    assert_eq!(op_of(&mut st, &["branch", "-D", "topic"]), Some(Op::DeleteBranch { name: "topic".into() }));
    for cmd in [&["commit", "-m", "x"][..], &["checkout", "topic"], &["fetch"], &["push"], &["stash", "push"], &["branch", "new"]] {
        assert_eq!(op_of(&mut st, cmd), None, "{cmd:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_note_survives_the_auto_stash_wrap() {
    let (mut st, dir) = opened("journal-git-job-stash");
    st.settings.auto_stash = true;
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    super::tests::drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Refresh));
    let fx = update(&mut st, Msg::git("Merge", &["merge", "topic"]), Instant::now());
    let Some(Effect::RunGit { cmds, journal, .. }) = fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) else { panic!("a job") };
    assert!(cmds.len() > 1, "stashed first: {cmds:?}");
    assert!(matches!(journal, Some(Op::Merge { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_job_in_another_repository_carries_none() {
    let (mut st, dir) = opened("journal-git-job-cwd");
    let mut fx = Vec::new();
    let spec = JobSpec { cwd: Some(dir.clone()), ..JobSpec::new("Pull".into(), vec![trellis_core::gitcli::args(&["pull"])]) };
    start_job(&mut st, spec, &mut fx);
    let Some(Effect::RunGit { journal, .. }) = fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) else { panic!("a job") };
    assert_eq!(journal, &None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The journal op of the first git job in `fx`.
fn journal_of(fx: &[Effect]) -> (usize, Option<Op>) {
    match fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) {
        Some(Effect::RunGit { cmds, journal, .. }) => (cmds.len(), journal.clone()),
        _ => panic!("a git job"),
    }
}

#[test]
fn a_guarded_pick_keeps_its_note_when_stashed_first() {
    let (mut st, dir) = opened("journal-pick-stash");
    st.settings.auto_stash = true;
    let git = |a: &[&str]| trellis_core::gitcli::read(&dir, &trellis_core::gitcli::args(a)).stdout.trim().to_owned();
    git(&["checkout", "-q", "topic"]);
    git(&["commit", "-q", "--allow-empty", "-m", "on topic"]);
    git(&["checkout", "-q", "-"]);
    let (pick, head) = (git(&["rev-parse", "topic"]), git(&["rev-parse", "HEAD"]));
    assert_ne!(pick, head);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    super::tests::drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Refresh));
    let oid = |rev: &str| git2::Oid::from_str(rev).unwrap();
    let fx = update(&mut st, Msg::Selection(crate::msg::SelectionMsg::CherryPick(vec![oid(&pick)])), Instant::now());
    let (n, op) = journal_of(&fx);
    assert!(n > 1, "stashed first");
    assert!(matches!(op, Some(Op::CherryPick { .. })), "{op:?}");
    st.job = None;
    st.auto_stash.making = None;
    let fx = update(&mut st, Msg::Selection(crate::msg::SelectionMsg::Revert(vec![oid(&head)])), Instant::now());
    let (n, op) = journal_of(&fx);
    assert!(n > 1, "stashed first");
    assert!(matches!(op, Some(Op::Revert { .. })), "{op:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recorded_job_of_several_commands_carries_its_note() {
    let (mut st, dir) = opened("journal-several");
    let t = trellis_core::repo::Tracking { remote: "origin".into(), merge: "topic".into() };
    let cmds = trellis_core::branch_ops::delete_with_remote_cmds("topic", &t);
    let record = Some(trellis_core::undo::UndoKind::DeleteBranch("topic".into()));
    let mut fx = Vec::new();
    start_job(&mut st, JobSpec { record, ..JobSpec::new("Delete local and remote branch".into(), cmds) }, &mut fx);
    assert_eq!(journal_of(&fx), (3, Some(Op::DeleteBranch { name: "topic".into() })));
    st.job = None;
    let cmds = vec![trellis_core::gitcli::args(&["add", "--all"]), trellis_core::gitcli::args(&["commit", "--amend", "-F", "msg"])];
    let record = Some(trellis_core::undo::UndoKind::Commit { amend: true });
    let mut fx = Vec::new();
    start_job(&mut st, JobSpec { record, ..JobSpec::new("Amend".into(), cmds) }, &mut fx);
    assert_eq!(journal_of(&fx), (2, Some(Op::Amend)));
    st.job = None;
    // A plain commit after staging everything is no rewrite.
    let cmds = vec![trellis_core::gitcli::args(&["add", "--all"]), trellis_core::gitcli::args(&["commit", "-F", "msg"])];
    let record = Some(trellis_core::undo::UndoKind::Commit { amend: false });
    let mut fx = Vec::new();
    start_job(&mut st, JobSpec { record, ..JobSpec::new("Stage all and commit".into(), cmds) }, &mut fx);
    assert_eq!(journal_of(&fx), (2, None));
    let _ = std::fs::remove_dir_all(&dir);
}
