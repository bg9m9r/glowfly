//! Glowfly's journal notes: the file, its lines, trimming, and capturing the refs around an
//! operation.

use super::notes::*;
use crate::gitcli::args;
use crate::testutil::*;
use crate::undo::{MoveOp, ResetMode, UndoKind};
use git2::Repository;
use std::path::Path;

fn commit(dir: &Path, text: &str) -> git2::Oid {
    write(dir, "a.txt", text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", text]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn every_op() -> Vec<Op> {
    let s = |x: &str| Some(x.to_owned());
    vec![
        Op::Rebase { onto: s("main") },
        Op::Rebase { onto: None },
        Op::Rewrite,
        Op::Merge { from: s("feature") },
        Op::Pull,
        Op::CherryPick { commits: s("3 commits") },
        Op::Revert { commits: None },
        Op::Reset { mode: "hard".into(), to: "1a2b3c4".into() },
        Op::Amend,
        Op::DeleteBranch { name: "old".into() },
        Op::Restore { branch: s("main"), to: "1a2b3c4".into() },
        Op::Restore { branch: None, to: "1a2b3c4".into() },
        Op::Undo { what: "hard reset to 1a2b3c4".into() },
        Op::Redo { what: "merge of feature".into() },
    ]
}

fn note(at: i64, op: Op) -> Note {
    Note {
        v: VERSION,
        at,
        op,
        refs: vec![NotedRef { branch: Some("main".into()), oid: "1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b".into() }],
    }
}

#[test]
fn every_kind_round_trips_through_the_file() {
    let dir = scratch_repo("journal-notes-round-trip");
    let notes: Vec<Note> = every_op().into_iter().enumerate().map(|(i, op)| note(i as i64, op)).collect();
    for n in &notes {
        append(&dir, n).unwrap();
    }
    assert_eq!(read(&dir), notes);
    assert!(dir.join(".git").join("glowfly").join("journal.jsonl").is_file(), "in the git dir");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn notes_written_as_trellis_move_to_the_glowfly_folder() {
    let dir = scratch_repo("journal-notes-trellis");
    let first = note(1, every_op().remove(0));
    append(&dir, &first).unwrap();
    let (new, old) = (dir.join(".git").join("glowfly"), dir.join(".git").join("trellis"));
    std::fs::rename(&new, &old).unwrap();
    assert_eq!(read(&dir), vec![first.clone()], "read from the old folder, moving it");
    assert!(new.join("journal.jsonl").is_file() && !old.exists());
    let second = note(2, every_op().remove(1));
    append(&dir, &second).unwrap();
    assert_eq!(read(&dir), vec![first, second]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_lines_are_skipped() {
    let good = serde_json::to_string(&note(1, Op::Amend)).unwrap();
    let newer = good.replace("\"v\":1", "\"v\":2");
    assert_ne!(newer, good);
    let unknown = good.replace("\"amend\"", "\"teleport\"");
    assert_ne!(unknown, good);
    let text = format!("{good}\n{newer}\n{unknown}\nnot json\n{good}\n{{\"v\":1,\"at\":");
    assert_eq!(parse(&text), vec![note(1, Op::Amend), note(1, Op::Amend)]);
    let dir = scratch_repo("journal-notes-missing");
    assert!(read(&dir).is_empty(), "no file, no notes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_large_file_is_trimmed_to_the_newest_notes() {
    let dir = scratch_repo("journal-notes-trim");
    let file = path(&dir).unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let what = "x".repeat(200);
    let mut text = String::new();
    for i in 0..1500 {
        text.push_str(&serde_json::to_string(&note(i, Op::Undo { what: what.clone() })).unwrap());
        text.push('\n');
    }
    assert!(text.len() as u64 > TRIM_AT);
    std::fs::write(&file, text).unwrap();
    append(&dir, &note(9999, Op::Amend)).unwrap();
    let notes = read(&dir);
    assert_eq!(notes.len(), MAX_NOTES);
    assert_eq!(notes.last().unwrap(), &note(9999, Op::Amend));
    assert_eq!(notes[0].at, 1500 - (MAX_NOTES as i64 - 1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_keeps_its_notes_in_its_own_git_dir() {
    let dir = scratch_repo("journal-notes-worktree");
    commit(&dir, "one");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &wt.display().to_string()]);
    append(&wt, &note(5, Op::Amend)).unwrap();
    let name = wt.file_name().unwrap();
    assert!(dir.join(".git").join("worktrees").join(name).join("glowfly").join("journal.jsonl").is_file());
    assert!(read(&dir).is_empty(), "the main worktree has its own notes");
    assert_eq!(read(&wt), vec![note(5, Op::Amend)]);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn capture_notes_the_state_before_a_move() {
    let dir = scratch_repo("journal-notes-capture");
    let c1 = commit(&dir, "one");
    let c2 = commit(&dir, "two");
    let op = Op::Reset { mode: "hard".into(), to: "HEAD~1".into() };
    let c = capture(&dir, op.clone(), 42).unwrap();
    assert!(!c.moved(&dir), "nothing ran yet");
    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);
    assert!(c.moved(&dir));
    let n = c.into_note();
    assert_eq!(n, Note { v: VERSION, at: 42, op, refs: vec![NotedRef { branch: Some("main".into()), oid: c2.to_string() }] });

    // A branch that isn't checked out: its own tip, not HEAD's.
    git(&dir, &["branch", "old", &c2.to_string()]);
    let c = capture(&dir, Op::DeleteBranch { name: "old".into() }, 43).unwrap();
    git(&dir, &["branch", "-D", "old"]);
    assert!(c.moved(&dir));
    assert_eq!(c.into_note().refs, vec![NotedRef { branch: Some("old".into()), oid: c2.to_string() }]);

    // Restoring HEAD itself notes HEAD, also on a branch.
    let c = capture(&dir, Op::Restore { branch: None, to: "x".into() }, 44).unwrap();
    git(&dir, &["checkout", "-q", "--detach", &c2.to_string()]);
    assert!(c.moved(&dir), "HEAD moved off main");
    assert_eq!(c.into_note().refs, vec![NotedRef { branch: None, oid: c1.to_string() }]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn of_kind_covers_the_rewriting_operations_only() {
    let cmd = args(&["reset", "--hard", "1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b"]);
    let mv = |op, d: Option<&str>| UndoKind::Move { op, detail: d.map(Into::into) };
    assert_eq!(Op::of_kind(&mv(MoveOp::Rebase, Some("main")), &[]), Some(Op::Rebase { onto: Some("main".into()) }));
    assert_eq!(Op::of_kind(&mv(MoveOp::Merge, Some("x")), &[]), Some(Op::Merge { from: Some("x".into()) }));
    assert_eq!(Op::of_kind(&mv(MoveOp::Pull, None), &[]), Some(Op::Pull));
    assert_eq!(Op::of_kind(&mv(MoveOp::CherryPick, Some("1a2b3c4")), &[]), Some(Op::CherryPick { commits: Some("1a2b3c4".into()) }));
    assert_eq!(Op::of_kind(&mv(MoveOp::Revert, None), &[]), Some(Op::Revert { commits: None }));
    assert_eq!(Op::of_kind(&UndoKind::Reset(ResetMode::Hard), &cmd), Some(Op::Reset { mode: "hard".into(), to: "1a2b3c4".into() }));
    let soft = args(&["reset", "--soft", "HEAD~1"]);
    assert_eq!(Op::of_kind(&UndoKind::Reset(ResetMode::Soft), &soft), Some(Op::Reset { mode: "soft".into(), to: "HEAD~1".into() }));
    assert_eq!(Op::of_kind(&UndoKind::Commit { amend: true }, &[]), Some(Op::Amend));
    assert_eq!(Op::of_kind(&UndoKind::DeleteBranch("old".into()), &[]), Some(Op::DeleteBranch { name: "old".into() }));
    for none in [
        UndoKind::Checkout,
        UndoKind::Commit { amend: false },
        UndoKind::CreateBranch("x".into()),
        UndoKind::CreateTag("v1".into()),
        UndoKind::StashPush { replay: None },
        UndoKind::StashApply { pop: true, rev: None },
        UndoKind::StashDrop { rev: None },
        UndoKind::DiscardAll,
    ] {
        assert_eq!(Op::of_kind(&none, &[]), None, "{none:?}");
    }
    assert_eq!(Op::DeleteBranch { name: "x".into() }.branch(), Some("x"));
    assert_eq!(Op::Restore { branch: Some("y".into()), to: "1".into() }.branch(), Some("y"));
    assert_eq!(Op::Restore { branch: None, to: "1".into() }.branch(), None);
    assert_eq!(Op::Amend.branch(), None);
}
