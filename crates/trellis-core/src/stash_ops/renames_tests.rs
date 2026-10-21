//! Stashing a chosen staged rename whole: finding its old path, the put-back commands, the
//! check that nothing on disk is in their way, and what they do to real repositories.

use super::push_tests::{committed, stdout, strings, text};
use super::*;
use crate::gitcli::run;
use crate::repo::{FileEntry, status_at};
use crate::testutil::{git, write};

#[test]
fn rename_sources_cases() {
    let renamed = |new: &str, old: Option<&str>| FileEntry { renamed_from: old.map(str::to_owned), ..FileEntry::new(new, 'R') };
    let staged =
        [renamed("new.txt", Some("old.txt")), FileEntry::new("x.txt", 'M'), renamed("b2.txt", Some("b.txt")), renamed("n.txt", None)];
    let sources = |paths: &[&str]| rename_sources(&strings(paths), &staged);
    assert_eq!(sources(&["new.txt", "y.txt"]), ["old.txt"]);
    assert!(sources(&["new.txt", "old.txt"]).is_empty(), "already chosen");
    assert_eq!(sources(&["b2.txt", "new.txt"]), ["b.txt", "old.txt"], "in the order of the paths");
    assert!(sources(&["n.txt"]).is_empty(), "the old name isn't known");
    assert!(sources(&["x.txt"]).is_empty());
    assert_eq!(sources(&["new.txt", "new.txt"]), ["old.txt"], "no duplicates");
}

#[test]
fn restore_commands_split_long_lists() {
    const RESET: [&str; 5] = ["--literal-pathspecs", "reset", "-q", "HEAD", "--"];
    assert!(restore_commands(&[]).is_empty());
    assert_eq!(
        restore_commands(&strings(&["a.txt", "b c.txt"])),
        [
            args(&["--literal-pathspecs", "reset", "-q", "HEAD", "--", "a.txt", "b c.txt"]),
            args(&["checkout-index", "-u", "--", "a.txt", "b c.txt"])
        ]
    );
    let many: Vec<String> = (0..300).map(|i| format!("{i:03}-{}", "x".repeat(56))).collect();
    let cmds = restore_commands(&many);
    assert!(cmds.len() > 2, "split into several pairs");
    for pair in cmds.chunks(2) {
        let [reset, put] = pair else { panic!("in pairs: {pair:?}") };
        assert_eq!(reset[..5], RESET);
        assert_eq!(put[..3], ["checkout-index", "-u", "--"]);
        assert_eq!(reset[5..], put[3..], "the same paths");
        assert!(put[3..].iter().map(|p| p.len() + 1).sum::<usize>() <= PATHS_ON_COMMAND_LINE);
    }
    assert_eq!(cmds.chunks(2).flat_map(|c| c[1][3..].iter().cloned()).collect::<Vec<_>>(), many);
}

#[test]
fn restore_blocked_sees_anything_on_disk() {
    let dir = committed("stash-push-restore-blocked");
    let blocked = |paths: &[&str]| restore_blocked(&dir, &restore_commands(&strings(paths)));
    assert_eq!(blocked(&["gone.txt", "new/deep/gone.txt"]), None, "free paths, folders made as needed");
    assert_eq!(blocked(&["gone.txt", "a.txt"]).as_deref(), Some("a.txt"));
    write(&dir, "plain", "a file, not a folder\n");
    assert_eq!(blocked(&["plain/x.txt"]).as_deref(), Some("plain/x.txt"), "a file in the way of the folder");
    std::fs::create_dir_all(dir.join("sub/dir")).unwrap();
    assert_eq!(blocked(&["sub/x.txt"]), None);
    assert_eq!(blocked(&["sub/dir"]).as_deref(), Some("sub/dir"), "a folder at the path");
    assert_eq!(blocked(&["../outside.txt"]).as_deref(), Some("../outside.txt"), "a path that can't be reached");
    // Only the put-back commands count, never the other commands of a job.
    assert_eq!(restore_blocked(&dir, &[args(&["checkout-index", "--", "a.txt"]), args(&["add", "--", "a.txt"])]), None);
    // Where the volume ignores case, a case twin is the same file to the system.
    write(&dir, "Twin.txt", "twin\n");
    let folds = dir.join("twin.txt").symlink_metadata().is_ok();
    assert_eq!(blocked(&["twin.txt"]).is_some(), folds);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_never_writes_over_a_file_at_the_old_path() {
    let dir = committed("stash-push-rename-occupied");
    git(&dir, &["mv", "a.txt", "moved.txt"]);
    let c = push_command(StashScope::Files, false, None, &strings(&["moved.txt"]), &dir.join(".git/unused"));
    let (ok, out) = run(&dir, &c.cmd);
    assert!(ok, "{out}");
    // A file that appeared at a.txt after the check (or one it couldn't see).
    write(&dir, "a.txt", "precious\n");
    let cmds = restore_commands(&strings(&["a.txt"]));
    assert_eq!(restore_blocked(&dir, &cmds).as_deref(), Some("a.txt"));
    let results: Vec<bool> = cmds.iter().map(|cmd| run(&dir, cmd).0).collect();
    assert_eq!(results, [true, false], "git refuses to write over it");
    assert_eq!(text(&dir, "a.txt"), "precious\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_rename_is_stashed_whole() {
    let dir = committed("stash-push-rename");
    git(&dir, &["mv", "a.txt", "moved.txt"]);
    let (_, staged, _) = status_at(&dir, true).unwrap();
    let moved = staged.iter().find(|f| f.path == "moved.txt").expect("the staged rename");
    assert_eq!((moved.status, moved.renamed_from.as_deref()), ('R', Some("a.txt")));
    let paths = strings(&["moved.txt"]);
    let old = rename_sources(&paths, &staged);
    assert_eq!(old, ["a.txt"]);

    let c = push_command(StashScope::Files, false, None, &paths, &dir.join(".git/unused"));
    for cmd in std::iter::once(c.cmd).chain(restore_commands(&old)) {
        let (ok, out) = run(&dir, &cmd);
        assert!(ok, "{cmd:?}: {out}");
    }
    assert_eq!(text(&dir, "a.txt"), "a\n");
    assert!(!dir.join("moved.txt").exists());
    assert_eq!(stdout(&dir, &["diff", "--cached", "--name-only"]).trim(), "", "nothing is left staged");
    let (unstaged, staged, _) = status_at(&dir, true).unwrap();
    assert!(unstaged.is_empty() && staged.is_empty(), "{unstaged:?} {staged:?}");

    git(&dir, &["stash", "pop", "--index"]);
    let status = stdout(&dir, &["status", "--porcelain"]);
    assert!(status.starts_with("R  a.txt -> moved.txt"), "the staged rename is back: {status}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keep_index_keeps_a_staged_rename_whole() {
    let dir = committed("stash-push-rename-keep");
    git(&dir, &["mv", "a.txt", "moved.txt"]);
    let c = push_command(StashScope::Files, true, None, &strings(&["moved.txt"]), &dir.join(".git/unused"));
    let (ok, out) = run(&dir, &c.cmd);
    assert!(ok, "{out}");
    assert!(stdout(&dir, &["status", "--porcelain"]).starts_with("R  a.txt -> moved.txt"), "still staged");
    assert_eq!(
        stdout(&dir, &["stash", "show", "--name-status", "stash@{0}"]).split_whitespace().collect::<Vec<_>>(),
        ["R100", "a.txt", "moved.txt"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_blocked_output_names_the_path() {
    let out = restore_blocked_output("dir/a.txt");
    assert!(out.starts_with(RESTORE_BLOCKED), "{out}");
    assert!(out.contains("dir/a.txt, where the stash would put back"), "{out}");
}
