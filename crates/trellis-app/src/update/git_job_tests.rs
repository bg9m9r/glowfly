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
