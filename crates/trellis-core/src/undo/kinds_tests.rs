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
        (&["checkout", "-b", "x"], None),
        (&["checkout", "-b", "x", sha], None),
        (&["checkout", "--", "a.txt"], None),
        (&["commit", "-F", "f"], Some(UndoKind::Commit { amend: false })),
        (&["commit", "-F", "f", "--amend"], Some(UndoKind::Commit { amend: true })),
        (&["reset", "--soft", sha], Some(UndoKind::Reset(ResetMode::Soft))),
        (&["reset", "--mixed", sha], Some(UndoKind::Reset(ResetMode::Mixed))),
        (&["reset", "--hard", sha], Some(UndoKind::Reset(ResetMode::Hard))),
        (&["branch", "-d", "x"], Some(UndoKind::DeleteBranch("x".into()))),
        (&["branch", "-D", "x"], Some(UndoKind::DeleteBranch("x".into()))),
        (&["branch", "x"], None),
        (&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE], Some(UndoKind::DiscardAll)),
        (&["stash", "push", "-u"], None),
        (&["merge", "x"], None),
        (&["pull", "--ff"], None),
        (&["fetch", "--all"], None),
    ];
    for (argv, want) in table {
        assert_eq!(&UndoKind::classify(&args(argv)), want, "{argv:?}");
    }
    assert_eq!(discard_command(), args(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]));
}
