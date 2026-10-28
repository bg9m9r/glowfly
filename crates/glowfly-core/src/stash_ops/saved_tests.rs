//! The saved auto-stash records: the file's round trip, what reads as none, and where it lives.

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

fn s(v: &str) -> String {
    v.to_owned()
}

fn committed(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn file(dir: &Path) -> PathBuf {
    dir.join(".git").join("glowfly").join("auto-stashes.json")
}

#[test]
fn records_round_trip_and_an_empty_list_removes_the_file() {
    let dir = committed("saved-auto-round-trip");
    let merge = AutoStashFor::Merge { from: s("feature"), into: s("main") };
    let pr = AutoStashFor::PullRequestCheckout { pr: s("#12"), branch: s("feature") };
    let saved = vec![
        SavedAutoStash::new(merge.clone(), merge.message(), Oid::from_bytes(&[1; 20]).unwrap()),
        SavedAutoStash::new(pr.clone(), pr.message(), Oid::from_bytes(&[2; 20]).unwrap()),
    ];
    assert_eq!(saved_path(&dir).unwrap(), file(&dir));
    write_saved(&dir, &saved).unwrap();
    let text = std::fs::read_to_string(file(&dir)).unwrap();
    assert!(text.contains("\"kind\": \"merge\"") && text.contains("\"kind\": \"pull_request_checkout\""), "{text}");
    assert_eq!(read_saved(&dir), saved);
    assert_eq!(saved[0].oid(), Some(Oid::from_bytes(&[1; 20]).unwrap()));
    write_saved(&dir, &[]).unwrap();
    assert!(!file(&dir).exists());
    write_saved(&dir, &[]).unwrap();
    assert!(read_saved(&dir).is_empty(), "a missing file reads as none");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn garbled_files_and_unknown_entries_are_skipped() {
    let dir = committed("saved-auto-garbled");
    std::fs::create_dir_all(file(&dir).parent().unwrap()).unwrap();
    let oid = "ab".repeat(20);
    let good = format!(r#"{{"op":{{"kind":"pull","branch":"main"}},"message":"Auto-stash before pulling into main","oid":"{oid}"}}"#);
    for text in [s("not json"), format!(r#"{{"v":2,"stashes":[{good}]}}"#)] {
        std::fs::write(file(&dir), &text).unwrap();
        assert!(read_saved(&dir).is_empty(), "{text}");
    }
    let unknown = format!(r#"{{"op":{{"kind":"teleport","to":"x"}},"message":"m","oid":"{oid}"}}"#);
    let bad_oid = r#"{"op":{"kind":"pull","branch":"main"},"message":"m","oid":"xyz"}"#;
    std::fs::write(file(&dir), format!(r#"{{"v":1,"stashes":[{unknown},{bad_oid},{good}]}}"#)).unwrap();
    let read = read_saved(&dir);
    assert_eq!(read.len(), 1, "{read:?}");
    assert_eq!(read[0].op, AutoStashFor::Pull { branch: s("main") });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_keeps_its_records_in_its_own_git_dir() {
    let dir = committed("saved-auto-worktree");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &wt.display().to_string()]);
    let op = AutoStashFor::GitFlowStart { branch: s("feature/login") };
    write_saved(&wt, &[SavedAutoStash::new(op.clone(), op.message(), Oid::from_bytes(&[3; 20]).unwrap())]).unwrap();
    let name = wt.file_name().unwrap();
    assert!(dir.join(".git").join("worktrees").join(name).join("glowfly").join("auto-stashes.json").is_file());
    assert!(read_saved(&dir).is_empty(), "the main worktree has its own records");
    assert_eq!(read_saved(&wt).len(), 1);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}
