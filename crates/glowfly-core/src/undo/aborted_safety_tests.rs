//! A kept abort never loses what is on disk: files status doesn't show (ignored ones, a folder
//! where HEAD has a file), edits a clean filter hides, a failed abort command, and the index
//! flags of an intent-to-add entry; and old refs go unless an undo entry still names them.

use super::aborted_tests::{abort, conflicted, kept_refs, read, run_all, state, two_conflicts, undo};
use super::tests::commit;
use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use git2::{Repository, RepositoryState, Signature, Time};
use std::path::{Path, PathBuf};

/// HEAD has the file `lib` and `config.local`; other turns `lib` into the folder `lib/a.rs`,
/// deletes `config.local` and ignores it, and both change c.txt. Stopped merging other.
fn reshaped(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "lib", "a file\n");
    write(&dir, "config.local", "tracked config\n");
    commit(&dir, "c.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    git(&dir, &["rm", "-q", "lib", "config.local"]);
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    write(&dir, "lib/a.rs", "fn a() {}\n");
    write(&dir, ".gitignore", "config.local\ntarget/\n");
    commit(&dir, "c.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "c.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflict");
    assert_eq!(read(&dir, "lib/a.rs"), "fn a() {}\n");
    dir
}

#[test]
fn a_file_where_the_abort_needs_a_folder_refuses_it() {
    let dir = scratch_repo("abort-safe-ancestor");
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    write(&dir, "docs/x.md", "the only file in docs\n");
    commit(&dir, "c.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    git(&dir, &["rm", "-q", "docs/x.md"]);
    commit(&dir, "c.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "c.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflict");
    assert!(!dir.join("docs").exists(), "git removed the empty folder");
    write(&dir, "docs", "a file a tool wrote\n");
    let r = abort_keeping(&dir, Some(&[]), |_| panic!("nothing runs"));
    assert!(!r.ok && r.output.contains("docs is a file where the abort needs a folder"), "{}", r.output);
    assert!(r.output.starts_with(NOTHING_ABORTED));
    assert_eq!(state(&dir), RepositoryState::Merge);
    assert_eq!(read(&dir, "docs"), "a file a tool wrote\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_under_a_file_reads_as_absent() {
    // ENOTDIR on Unix, not found on Windows: either way nothing is there.
    let dir = scratch_repo("abort-safe-notdir");
    write(&dir, "lib", "a file\n");
    assert_eq!(super::stash::hash(&dir, "lib/a.rs").unwrap(), None);
    assert!(matches!(super::aborted::on_disk(&dir, "lib/a.rs").unwrap(), super::aborted::OnDisk::Nothing));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_holding_files_the_merge_didnt_bring_refuses_the_abort() {
    let dir = reshaped("abort-safe-folder");
    write(&dir, "lib/b.rs", "my new file\n");
    let r = abort_keeping(&dir, Some(&[]), |_| panic!("nothing runs"));
    assert!(!r.ok && r.output.contains("lib is a folder holding lib/b.rs"), "{}", r.output);
    assert_eq!(state(&dir), RepositoryState::Merge);
    assert_eq!(read(&dir, "lib/b.rs"), "my new file\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_of_the_merges_own_files_and_an_ignored_file_come_back() {
    let dir = reshaped("abort-safe-reshaped");
    write(&dir, "config.local", "my ignored config\n");
    let action = abort(&dir);
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(read(&dir, "lib"), "a file\n", "HEAD's file is back where the folder was");
    assert_eq!(read(&dir, "config.local"), "tracked config\n");

    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::Merge);
    assert_eq!(read(&dir, "lib/a.rs"), "fn a() {}\n");
    assert_eq!(read(&dir, "config.local"), "my ignored config\n", "the ignored file was kept");
    assert_eq!(conflicted(&dir), ["c.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_compares_bytes_not_status() {
    let dir = two_conflicts("abort-safe-bytes");
    let action = abort(&dir);
    // With autocrlf, CRLF line ends are hidden from status, but they are bytes undo would lose.
    git(&dir, &["config", "core.autocrlf", "true"]);
    std::fs::write(dir.join("a.txt"), "ours\r\n").unwrap();
    assert!(matches!(plan(&dir, &action, false).unwrap(), Plan::Refuse(Refusal::Dirty(_))));
    std::fs::write(dir.join("a.txt"), "ours\n").unwrap();
    assert_eq!(plan(&dir, &action, false).unwrap(), Plan::RestoreAbort);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_abort_command_puts_everything_back() {
    let dir = two_conflicts("abort-safe-rollback");
    write(&dir, "a.txt", "merged\n");
    git(&dir, &["add", "a.txt"]);
    let edited = read(&dir, "b.txt").replace("b ours", "b ours, edited");
    write(&dir, "b.txt", &edited);
    let merge_head = super::aborted_tests::git_file(&dir, "MERGE_HEAD");
    // The first command (the reset) runs; the next fails, as a locked file would.
    let r = abort_keeping(&dir, Some(&[]), |cmds| {
        let (ok, out) = run_all(&dir, &cmds[..1]);
        assert!(ok, "{out}");
        (false, "error: unable to unlink a.txt".into())
    });
    assert!(!r.ok && r.action.is_none());
    assert!(r.output.contains("everything it had was put back"), "{}", r.output);
    assert_eq!(state(&dir), RepositoryState::Merge);
    assert_eq!(super::aborted_tests::git_file(&dir, "MERGE_HEAD"), merge_head);
    assert_eq!((read(&dir, "a.txt"), read(&dir, "b.txt")), ("merged\n".into(), edited));
    assert_eq!(conflicted(&dir), ["b.txt"]);
    assert_eq!(kept_refs(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_intent_to_add_entry_comes_back_as_one() {
    let dir = two_conflicts("abort-safe-ita");
    write(&dir, "later.txt", "not staged yet\n");
    git(&dir, &["add", "-N", "later.txt"]);
    let action = abort(&dir);
    assert!(!dir.join("later.txt").exists());
    undo(&dir, &action);
    assert_eq!(read(&dir, "later.txt"), "not staged yet\n");
    let index = Repository::open(&dir).unwrap().index().unwrap();
    let e = index.get_path(Path::new("later.txt"), 0).expect("in the index");
    assert_ne!(e.flags_extended & 0x2000, 0, "still intent-to-add");
    let (_, out) = run(&dir, &args(&["status", "--porcelain"]));
    assert!(out.lines().any(|l| l == " A later.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_refs_go_unless_an_undo_entry_names_them() {
    let dir = two_conflicts("abort-safe-prune");
    let repo = Repository::open(&dir).unwrap();
    let old = Signature::new("Glowfly", "glowfly@localhost", &Time::new(1_000_000_000, 0)).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let make = |n: &str| {
        let oid = repo.commit(None, &old, &old, n, &tree, &[]).unwrap();
        repo.reference(&format!("{ABORT_REF_PREFIX}{oid}"), oid, true, "test").unwrap();
        oid
    };
    let (named, unnamed) = (make("named"), make("unnamed"));
    let action = abort_keeping(&dir, Some(&[named]), |cmds| run_all(&dir, cmds)).action.unwrap();
    let has = |o: git2::Oid| repo.find_reference(&format!("{ABORT_REF_PREFIX}{o}")).is_ok();
    assert!(has(named) && !has(unnamed));
    undo(&dir, &action);
    // A redo drops none.
    let _ = abort_keeping(&dir, None, |cmds| run_all(&dir, cmds));
    assert!(has(named));
    let _ = std::fs::remove_dir_all(&dir);
}
