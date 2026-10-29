//! Reading MERGE_MSG for the commit draft of a merge in progress, also in a legacy encoding.

use super::merge_msg::clean;
use super::*;
use crate::commit_encoding::MessageEncoding;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// A repository on `main` whose merge of `other` stops with a conflict in a.txt.
fn conflicted_merge(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "base\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "a.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "a.txt", "ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    start_merge(&dir);
    dir
}

/// `git merge other`, which stops for the conflict (so it exits non-zero).
fn start_merge(dir: &Path) {
    let (ok, _) = run(dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge should stop for a conflict");
}

fn loaded(dir: &Path) -> Option<String> {
    load(dir, &LoadOptions::all(10)).unwrap().merge_message
}

#[test]
fn a_conflicted_merge_reads_its_message_without_comments() {
    let dir = conflicted_merge("merge-msg");
    let raw = std::fs::read_to_string(dir.join(".git/MERGE_MSG")).unwrap();
    assert!(raw.contains("# Conflicts:"), "git lists the conflicts as comments: {raw:?}");
    assert_eq!(loaded(&dir).as_deref(), Some("Merge branch 'other'"));

    git(&dir, &["merge", "--abort"]);
    assert_eq!(loaded(&dir), None, "no merge in progress");
}

#[test]
fn cleaning_follows_the_comment_char_scissors_and_line_ends() {
    assert_eq!(clean("Merge x\n; note\n# kept\n", ';').as_deref(), Some("Merge x\n# kept"));
    assert_eq!(
        clean("Merge x\n\nbody\n# ------------------------ >8 ------------------------\ndiff\n", '#').as_deref(),
        Some("Merge x\n\nbody")
    );
    assert_eq!(clean("Merge x\r\n\r\nbody  \r\n", '#').as_deref(), Some("Merge x\n\nbody"));
    assert_eq!(clean(" \n# only a comment\n\n", '#'), None);
    assert_eq!(clean("", '#'), None);
}

#[test]
fn the_repository_comment_char_applies() {
    let dir = conflicted_merge("merge-msg-char");
    set_config(&dir, &[("core.commentChar", ";")]);
    std::fs::write(dir.join(".git/MERGE_MSG"), "Merge branch 'other'\r\n; gone\r\n# stays\r\n").unwrap();
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(merge_message(&repo, &MessageEncoding::UTF8).as_deref(), Some("Merge branch 'other'\n# stays"));
    std::fs::write(dir.join(".git/MERGE_MSG"), "\n; nothing\n").unwrap();
    assert_eq!(merge_message(&repo, &MessageEncoding::UTF8), None, "blank once cleaned");
}

#[test]
fn a_linked_worktree_reads_its_own_merge_message() {
    let main = scratch_repo("merge-msg-wt");
    write(&main, "a.txt", "base\n");
    git(&main, &["add", "."]);
    git(&main, &["commit", "-q", "-m", "base"]);
    git(&main, &["branch", "other"]);
    let linked = main.with_file_name(format!("{}-linked", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    git(&main, &["checkout", "-q", "other"]);
    write(&main, "a.txt", "theirs\n");
    git(&main, &["commit", "-q", "-am", "theirs"]);
    git(&main, &["checkout", "-q", "main"]);
    write(&linked, "a.txt", "side\n");
    git(&linked, &["commit", "-q", "-am", "side"]);
    start_merge(&linked);

    assert_eq!(loaded(&linked).as_deref(), Some("Merge branch 'other' into side"));
    assert_eq!(loaded(&main), None, "the main worktree has no merge in progress");
}

#[test]
fn a_cherry_pick_message_in_the_commit_encoding_reads_decoded() {
    let dir = scratch_repo("merge-msg-1252");
    set_config(&dir, &[("i18n.commitEncoding", "windows-1252")]);
    write(&dir, "a.txt", "base\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    // git writes a stopped cherry-pick's MERGE_MSG in the commit encoding: not UTF-8.
    std::fs::write(dir.join(".git/MERGE_MSG"), b"Caf\xe9 cr\xe8me\n\n# Conflicts:\n#\ta.txt\n").unwrap();
    let repo = Repository::open(&dir).unwrap();
    let enc = MessageEncoding::of(&repo);
    assert_eq!(merge_message(&repo, &enc).as_deref(), Some("Café crème"));
    assert_eq!(merge_message(&repo, &MessageEncoding::UTF8).as_deref(), Some("Caf\u{FFFD} cr\u{FFFD}me"), "lossy, never a panic");
}
