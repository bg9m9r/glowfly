//! The command lines `UndoKind::classify` recognises, and the ones it leaves alone.

use super::*;

#[test]
fn classify_recognises_trellis_commands() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let table: &[(&[&str], Option<UndoKind>)] = &[
        (&["checkout", "main"], Some(UndoKind::Checkout)),
        (&["checkout", "main", "--"], Some(UndoKind::Checkout)),
        (&["checkout", "--track", "origin/x", "--"], Some(UndoKind::Checkout)),
        (&["checkout", "main", "--", "a.txt"], None),
        (&["checkout", "--track", "origin/x"], Some(UndoKind::Checkout)),
        (&["checkout", sha], Some(UndoKind::Checkout)),
        (&["checkout", "-b", "x"], Some(UndoKind::Checkout)),
        (&["checkout", "-b", "x", sha], Some(UndoKind::Checkout)),
        (&["checkout", "-b", "x", sha, "--"], Some(UndoKind::Checkout)),
        (&["checkout", "-B", "x"], None),
        (&["checkout", "-b"], None),
        (&["checkout", "-b", "--track", "x"], None),
        (&["checkout", "--", "a.txt"], None),
        (&["commit", "-F", "f"], Some(UndoKind::Commit { amend: false })),
        (&["commit", "-F", "f", "--amend"], Some(UndoKind::Commit { amend: true })),
        (&["reset", "--soft", sha], Some(UndoKind::Reset(ResetMode::Soft))),
        (&["reset", "--mixed", sha], Some(UndoKind::Reset(ResetMode::Mixed))),
        (&["reset", "--hard", sha], Some(UndoKind::Reset(ResetMode::Hard))),
        (&["branch", "-d", "x"], Some(UndoKind::DeleteBranch("x".into()))),
        (&["branch", "-D", "x"], Some(UndoKind::DeleteBranch("x".into()))),
        (&["branch", "x"], Some(UndoKind::CreateBranch("x".into()))),
        (&["branch", "x", sha], Some(UndoKind::CreateBranch("x".into()))),
        (&["branch", "-m", "a", "b"], None),
        (&["branch", "--unset-upstream", "x"], None),
        (&["branch", "--set-upstream-to=o/x", "x"], None),
        (&["tag", "v1", sha], Some(UndoKind::CreateTag("v1".into()))),
        (&["tag", "-a", "v1", sha, "-m", "m"], Some(UndoKind::CreateTag("v1".into()))),
        (&["tag", "-d", "v1"], None),
        (&["tag", "-a", "-f", "v1", sha, "-m", "m"], None),
        (&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE], Some(UndoKind::DiscardAll)),
        (&["merge", "x"], Some(UndoKind::Move { op: MoveOp::Merge, detail: Some("x".into()) })),
        (
            &["merge", "--ff-only", "refs/remotes/origin/x"],
            Some(UndoKind::Move { op: MoveOp::Merge, detail: Some("refs/remotes/origin/x".into()) }),
        ),
        (&["merge", "--squash", "x"], None),
        (&["merge", "--abort"], None),
        (&["merge", "--no-commit", "x"], None),
        (&["pull"], Some(UndoKind::Move { op: MoveOp::Pull, detail: None })),
        (&["pull", "--ff"], Some(UndoKind::Move { op: MoveOp::Pull, detail: None })),
        (&["pull", "--rebase"], Some(UndoKind::Move { op: MoveOp::Pull, detail: None })),
        (&["cherry-pick", sha], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("0123456".into()) })),
        (&["cherry-pick", "a", "b"], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("2 commits".into()) })),
        (&["cherry-pick", "--continue"], Some(UndoKind::Resume(MoveOp::CherryPick))),
        (&["cherry-pick", "--abort"], None),
        (&["cherry-pick", "-n", "a"], None),
        (&["revert", "--no-edit", sha], Some(UndoKind::Move { op: MoveOp::Revert, detail: Some("0123456".into()) })),
        (&["revert", "--abort"], None),
        (&["revert", "--quit"], None),
        (&["revert", "--continue"], Some(UndoKind::Resume(MoveOp::Revert))),
        (&["merge", "--continue"], Some(UndoKind::Resume(MoveOp::Merge))),
        (&["merge", "--skip"], None),
        (&["cherry-pick", "-m", "1", sha], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("0123456".into()) })),
        (&["cherry-pick", "--mainline", "2", sha], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("0123456".into()) })),
        (&["cherry-pick", "--mainline=2", sha], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("0123456".into()) })),
        (&["cherry-pick", "-m2", sha], Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("0123456".into()) })),
        (&["revert", "--no-edit", "-m", "1", sha], Some(UndoKind::Move { op: MoveOp::Revert, detail: Some("0123456".into()) })),
        (&["revert", "--no-edit", "a", "b", "c"], Some(UndoKind::Move { op: MoveOp::Revert, detail: Some("3 commits".into()) })),
        (&["cherry-pick", "--skip"], Some(UndoKind::Resume(MoveOp::CherryPick))),
        (&["revert", "--skip"], Some(UndoKind::Resume(MoveOp::Revert))),
        (&["rebase", "main"], Some(UndoKind::Move { op: MoveOp::Rebase, detail: Some("main".into()) })),
        (&["rebase", "-i", "main"], None),
        (&["rebase", "--interactive", "main"], None),
        (&["rebase", "--continue"], None),
        (&["rebase", "--edit-todo"], None),
        (&["stash", "push", "-u"], Some(UndoKind::StashPush { replay: Some(args(&["stash", "push", "-u"])) })),
        (&["stash", "push", "-u", "-m", "m"], Some(UndoKind::StashPush { replay: Some(args(&["stash", "push", "-u", "-m", "m"])) })),
        (
            &["--literal-pathspecs", "stash", "push", "-u", "--", "a.txt"],
            Some(UndoKind::StashPush { replay: Some(args(&["--literal-pathspecs", "stash", "push", "-u", "--", "a.txt"])) }),
        ),
        (
            &["--literal-pathspecs", "stash", "push", "-u", "-m", "m", "--pathspec-from-file=f", "--pathspec-file-nul"],
            Some(UndoKind::StashPush { replay: None }),
        ),
        (&["stash", "pop"], Some(UndoKind::StashApply { pop: true, rev: None })),
        (&["stash", "pop", "stash@{2}"], Some(UndoKind::StashApply { pop: true, rev: Some("stash@{2}".into()) })),
        (&["stash", "apply", "stash@{0}"], Some(UndoKind::StashApply { pop: false, rev: Some("stash@{0}".into()) })),
        (&["stash", "pop", "--index"], None),
        (&["stash", "drop"], Some(UndoKind::StashDrop { rev: None })),
        (&["stash", "drop", "stash@{1}"], Some(UndoKind::StashDrop { rev: Some("stash@{1}".into()) })),
        (&["stash", "drop", "-q", "stash@{1}"], None),
        (&["fetch", "--all"], None),
    ];
    for (argv, want) in table {
        assert_eq!(&UndoKind::classify(&args(argv)), want, "{argv:?}");
    }
    assert_eq!(discard_command(), args(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]));
}

#[test]
fn rewrite_is_never_classified() {
    assert_eq!(MoveOp::Rewrite.noun(), "history edit");
    for argv in [&["rebase", "-i", "main"][..], &["rebase", "main"], &["rebase", "--continue"], &["rebase", "--edit-todo"]] {
        let kind = UndoKind::classify(&args(argv));
        assert!(!matches!(kind, Some(UndoKind::Move { op: MoveOp::Rewrite, .. })), "{argv:?}");
    }
}
