use super::*;
use crate::state::op_fixture::{rows, snapshot};
use glowfly_core::rebase::{RebaseProgress, Stop};

fn merging(name: &str, conflicted: bool) -> Snapshot {
    let mut snap = snapshot(name);
    snap.state = RepositoryState::Merge;
    snap.unstaged = if conflicted { rows(&[("a.txt", '!'), ("b.txt", 'M')]) } else { rows(&[("b.txt", 'M')]) };
    snap
}

#[test]
fn every_operation_has_steps_and_bisect_keeps_its_card() {
    use RepositoryState::*;
    let mut snap = snapshot("op-steps-kinds");
    for (state, kind) in [
        (Merge, Some(OpKind::Merge)),
        (Rebase, Some(OpKind::Rebase)),
        (RebaseInteractive, Some(OpKind::Rebase)),
        (CherryPickSequence, Some(OpKind::CherryPick)),
        (Revert, Some(OpKind::Revert)),
        (ApplyMailbox, Some(OpKind::Patches)),
        (Bisect, None),
        (ApplyMailboxOrRebase, None),
        (Clean, None),
    ] {
        snap.state = state;
        assert_eq!(op_kind(&snap), kind, "{state:?}");
    }
    snap.unstaged = rows(&[("a.txt", '!')]);
    snap.state = Clean;
    assert_eq!(op_kind(&snap), Some(OpKind::Conflicts), "a stash that conflicted");
}

#[test]
fn steps_move_on_once_the_conflicts_are_resolved() {
    use OpStep::*;
    assert_eq!([Resolve, Check, Finish].map(|s| step_state(s, 2)), [StepState::Now, StepState::Later, StepState::Later]);
    assert_eq!([Resolve, Check, Finish].map(|s| step_state(s, 0)), [StepState::Done, StepState::Done, StepState::Now]);

    let snap = merging("op-steps-open-conflicted", true);
    assert_eq!([Resolve, Check, Finish].map(|s| default_open(&snap, s)), [true, true, false]);
    let snap = merging("op-steps-open-ready", false);
    assert_eq!([Resolve, Check, Finish].map(|s| default_open(&snap, s)), [false, false, true]);
}

#[test]
fn a_rebase_paused_to_edit_opens_the_commit_form() {
    let mut snap = snapshot("op-steps-edit");
    snap.state = RepositoryState::RebaseInteractive;
    let progress = |kind| RebaseProgress {
        step: 3,
        total: 7,
        stop: Stop { kind, oid: None },
        summary: String::new(),
        autostash: false,
        interactive: true,
        command: None,
    };
    snap.rebase = Some(progress(StopKind::Edit));
    assert!(default_open(&snap, OpStep::CommitForm));
    snap.rebase = Some(progress(StopKind::Conflict));
    assert!(!default_open(&snap, OpStep::CommitForm), "a conflict stop continues, it doesn't commit");
    assert_eq!(finish_title(Lang::En, &snap, OpKind::Rebase), "Continue rebase \u{b7} step 3 of 7");
    assert_ne!(finish_title(Lang::De, &snap, OpKind::Rebase), finish_title(Lang::En, &snap, OpKind::Rebase));
}

#[test]
fn a_chosen_fold_lasts_until_the_operation_moves_on() {
    let mut folds = StepFolds::default();
    let conflicted = merging("op-steps-fold", true);
    assert!(!folds.is_open(&conflicted, OpStep::Finish));
    folds.toggle(&conflicted, OpStep::Finish);
    assert!(folds.is_open(&conflicted, OpStep::Finish), "opened by hand");
    folds.toggle(&conflicted, OpStep::Resolve);
    assert!(!folds.is_open(&conflicted, OpStep::Resolve), "folded by hand");
    assert!(folds.is_open(&conflicted, OpStep::Finish), "the other choice stays");

    // Every conflict resolved: the defaults again, with step 3 open.
    let ready = merging("op-steps-fold-ready", false);
    assert!(folds.is_open(&ready, OpStep::Finish));
    assert!(!folds.is_open(&ready, OpStep::Resolve));
    folds.toggle(&ready, OpStep::Resolve);
    assert!(folds.is_open(&ready, OpStep::Resolve), "a done step reopens");
}

#[test]
fn words_per_operation() {
    let t = Lang::En;
    let snap = merging("op-steps-words", true);
    assert_eq!(heading(t, &snap), "Merge in progress");
    assert_eq!(finish_title(t, &snap, OpKind::Merge), "Commit the merge");
    assert_eq!(finish_title(t, &snap, OpKind::CherryPick), "Continue the cherry-pick");
    assert_eq!(finish_title(t, &snap, OpKind::Revert), "Continue the revert");
    assert_eq!(resolve_line(t, 2, 0), "2 left");
    assert_eq!(resolve_line(t, 0, 1), "Conflicts resolved");
    assert_eq!(resolve_line(t, 0, 0), "No conflicts");
    assert_eq!(check_line(t, 1, 2), "Staged: 1 \u{b7} Left out: 2");
    for lang in [Lang::De, Lang::Es] {
        assert_ne!(finish_title(lang, &snap, OpKind::Merge), "Commit the merge", "{lang:?}");
        assert_ne!(check_note(lang, OpKind::Merge), check_note(t, OpKind::Merge), "{lang:?}");
    }
}
