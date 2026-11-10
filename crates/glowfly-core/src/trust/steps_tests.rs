//! Which steps left in a rebase, cherry-pick or revert in progress the scan lists: the todo's
//! `exec` steps and a merge strategy that isn't git's own, and how a decision covers them.

use super::super::{allows, scan_at};
use crate::settings::SavedTrust;
use crate::testutil::*;
use std::path::Path;

/// Writes `text` as the file `name` in the git dir of the repository at `dir`.
fn state_file(dir: &Path, name: &str, text: &[u8]) {
    let path = dir.join(".git").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn steps(dir: &Path) -> Vec<String> {
    scan_at(dir).unwrap().steps
}

#[test]
fn a_todo_with_exec_steps_is_a_reason_to_ask() {
    let dir = scratch_repo("trust-steps-exec");
    let todo = "pick 1234567 one\nexec make test\n\tx\ttouch done.txt  \r\n# exec in a comment\nbreak\nexec make test\n";
    state_file(&dir, "rebase-merge/git-rebase-todo", todo.as_bytes());
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.steps, ["exec make test", "exec touch done.txt"]);
    assert!(!s.is_empty());
    assert!(!allows(None, &s), "nothing was decided, and continuing runs a command");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_todo_without_exec_steps_runs_nothing() {
    let dir = scratch_repo("trust-steps-none");
    let todo = "pick 1234567 exec is only a word here\nreword 89abcde x\nlabel exec\nbreak\nupdate-ref refs/heads/x\n# exec make\n";
    state_file(&dir, "rebase-merge/git-rebase-todo", todo.as_bytes());
    state_file(&dir, "sequencer/todo", b"pick 1234567 one\nrevert 89abcde two\n");
    state_file(&dir, "rebase-merge/strategy", b"ort\n");
    state_file(&dir, "sequencer/opts", b"[options]\n\tstrategy = \"recursive\"\n\tstrategy-option = theirs\n\tmainline = 1\n");
    let s = scan_at(&dir).unwrap();
    assert!(s.steps.is_empty() && s.is_empty(), "{s:?}");
    assert!(allows(None, &s));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pick_sequences_exec_steps_and_bytes_that_arent_utf8_are_listed() {
    let dir = scratch_repo("trust-steps-sequencer");
    state_file(&dir, "sequencer/todo", b"pick 1234567 one\nexec sh \xff.sh\nexec\n");
    assert_eq!(steps(&dir), ["exec sh \u{FFFD}FF.sh", "exec"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_strategy_that_isnt_gits_own_is_listed() {
    let dir = scratch_repo("trust-steps-strategy");
    state_file(&dir, "rebase-merge/strategy", b"mine\n");
    assert_eq!(steps(&dir), ["--strategy=mine"]);
    // Git compares the name as written: another letter case or a space is another program.
    state_file(&dir, "rebase-merge/strategy", b" ORT\n");
    assert_eq!(steps(&dir), ["--strategy= ORT"]);
    state_file(&dir, "rebase-merge/strategy", b"");
    assert_eq!(steps(&dir), ["--strategy="]);
    std::fs::remove_dir_all(dir.join(".git/rebase-merge")).unwrap();
    state_file(&dir, "sequencer/opts", b"[options] strategy = ort\n[options]\n\tStrategy=theirs-first\n");
    assert_eq!(steps(&dir), ["--strategy=theirs-first"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_decision_covers_the_steps_it_was_made_on_and_asks_again_for_a_new_one() {
    let dir = scratch_repo("trust-steps-print");
    state_file(&dir, "rebase-merge/git-rebase-todo", b"exec make test\npick 1234567 one\n");
    let seen = scan_at(&dir).unwrap();
    let saved = SavedTrust::trusted(Some(seen.print()));
    assert!(allows(Some(&saved), &scan_at(&dir).unwrap()), "nothing changed since the decision");
    // The step ran and left the todo: still covered.
    state_file(&dir, "rebase-merge/git-rebase-todo", b"pick 1234567 one\n");
    assert!(allows(Some(&saved), &scan_at(&dir).unwrap()));
    // The todo gained a command since.
    state_file(&dir, "rebase-merge/git-rebase-todo", b"exec make test\npick 1234567 one\nexec make install\n");
    let now = scan_at(&dir).unwrap();
    assert!(!allows(Some(&saved), &now));
    let gained = now.gained_since(&seen.print());
    assert_eq!(gained.steps, ["exec make install"]);
    assert!(!gained.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
