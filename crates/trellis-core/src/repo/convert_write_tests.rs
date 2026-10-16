//! The other writers on files git converts (the `filter=upper` fixture from `convert_tests`:
//! stored upper-case, checked out lower-case): resolving a conflict, restoring from a commit,
//! staged renames and deleting a staged new file. Each stores the repository form and leaves
//! the working tree as git's smudge writes it.

use super::convert_tests::{filtered_repo, index_text, read};
use super::*;
use crate::error::Error;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

fn stage_id(dir: &Path, path: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().index().unwrap().get_path(Path::new(path), 0).map(|e| e.id)
}

/// `a.up` in conflict: `main` and `other` each changed line 5 of the committed base.
fn conflicted(name: &str) -> PathBuf {
    let dir = filtered_repo(name);
    git(&dir, &["checkout", "--", "a.up"]);
    let base = read(&dir, "a.up");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "a.up", &base.replace("line 5\n", "line five theirs\n"));
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "a.up", &base.replace("line 5\n", "line five ours\n"));
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "-q", "other"]));
    assert!(!ok, "the merge should conflict");
    dir
}

#[test]
fn write_resolution_stores_the_repository_form_and_smudges_the_working_file() {
    let dir = conflicted("cvw-resolution");
    let file = load_conflict(&dir, "a.up").unwrap();
    assert_eq!(file.chunk_count(), 1);
    let out = render_output(&file, &[Choice::OursThenTheirs]).unwrap();
    assert!(out.contains("LINE FIVE OURS\nLINE FIVE THEIRS\n"), "merged from the stored stages: {out}");
    write_resolution(&dir, "a.up", &out, &file.ids).unwrap();
    assert_eq!(index_text(&dir, "a.up"), out, "staged as is");
    let text = read(&dir, "a.up");
    assert!(text.contains("line five ours\nline five theirs\n") && text.starts_with("line 1\n"), "checked out through the smudge: {text}");
    assert_eq!(status(&dir), "M  a.up", "resolved, with no unstaged change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_with_side_stages_the_side_and_smudges_the_working_file() {
    let dir = conflicted("cvw-side");
    let file = load_conflict(&dir, "a.up").unwrap();
    resolve_with_side(&dir, "a.up", Side::Theirs, &file.ids).unwrap();
    assert_eq!(stage_id(&dir, "a.up"), file.ids.theirs, "their blob, unchanged");
    let text = read(&dir, "a.up");
    assert!(text.contains("line five theirs\n") && !text.contains("LINE"), "checked out through the smudge: {text}");
    assert_eq!(status(&dir), "M  a.up");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_file_runs_the_smudge_filter() {
    let dir = filtered_repo("cvw-restore");
    let base = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    git(&dir, &["commit", "-q", "-am", "edit"]);
    let staged = index_text(&dir, "a.up");
    assert_eq!(restore_file(&dir, base, "a.up", false).unwrap(), RestoredFrom::Commit);
    assert_eq!(read(&dir, "a.up"), (1..=20).map(|i| format!("line {i}\n")).collect::<String>(), "smudged");
    assert_eq!(index_text(&dir, "a.up"), staged, "the index is left alone");
    assert_eq!(status(&dir), " M a.up");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_file_sees_a_clean_filtered_file_with_stale_stat_data_as_clean() {
    let dir = filtered_repo("cvw-restore-stale");
    git(&dir, &["checkout", "--", "a.up"]);
    let base = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    write(&dir, "a.up", &read(&dir, "a.up").replace("line 3\n", "line three\n"));
    git(&dir, &["commit", "-q", "-am", "edit"]);
    // Same contents, older mtime: the index's stat data no longer matches the file.
    let file = std::fs::File::options().write(true).open(dir.join("a.up")).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
    drop(file);
    assert_eq!(restore_file(&dir, base, "a.up", false).unwrap(), RestoredFrom::Commit, "a clean file is no reason to ask");
    assert_eq!(read(&dir, "a.up"), (1..=20).map(|i| format!("line {i}\n")).collect::<String>());
    write(&dir, "a.up", "a local edit\n");
    assert!(matches!(restore_file(&dir, base, "a.up", false), Err(Error::WouldOverwrite(_))), "a real edit still asks");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_file_staged_keeps_the_stored_blob_and_the_working_bytes() {
    let dir = filtered_repo("cvw-rename");
    git(&dir, &["checkout", "--", "a.up"]);
    let (blob, text) = (stage_id(&dir, "a.up"), read(&dir, "a.up"));
    rename_file_staged(&dir, "a.up", "b.up").unwrap();
    assert_eq!(stage_id(&dir, "b.up"), blob);
    assert_eq!(read(&dir, "b.up"), text);
    assert_eq!(status(&dir), "R  a.up -> b.up", "a clean rename, nothing unstaged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleting_a_staged_new_filtered_file_takes_it_out_of_the_index() {
    let dir = filtered_repo("cvw-delete-new");
    git(&dir, &["checkout", "--", "a.up"]);
    write(&dir, "n.up", "fresh\n");
    stage_file(&dir, "n.up").unwrap();
    assert_eq!(index_text(&dir, "n.up"), "FRESH\n");
    delete_staged_new_file(&dir, "n.up", Removal::Permanent).unwrap();
    assert!(stage_id(&dir, "n.up").is_none() && !dir.join("n.up").exists());
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}
