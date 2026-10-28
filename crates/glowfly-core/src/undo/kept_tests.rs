//! The journal's records of discarded work against real repositories: a discard, delete or
//! stash drop keeps one (none when nothing changed), it stays reachable for `git gc`, the list
//! hides what is back, Restore never writes over anything, and old records are pruned.

use super::kept::{self, KeptItem, KeptWhat, Restored};
use super::test_bin::T;
use super::*;
use crate::gitcli::args;
use crate::repo;
use crate::testkit::git_out;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    write(&dir, "f.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

fn discard(dir: &Path) -> Action {
    discard_saving(dir, "f.txt", Discarded::File, || repo::discard_file(dir, "f.txt")).unwrap().expect("undoable")
}

#[test]
fn a_discard_keeps_its_bytes_reachable_and_listed() {
    let dir = base("kept-discard");
    write(&dir, "f.txt", "edited\n");
    let a = discard(&dir);
    let Action::Discard { before: Some(before), .. } = a else { panic!("{a:?}") };
    let list = kept::list(&dir).unwrap();
    let [k] = &list[..] else { panic!("one record: {list:?}") };
    assert_eq!(k.what, KeptWhat::File);
    assert_eq!(k.item, KeptItem::File { path: "f.txt".into(), blob: before.blob, executable: false });
    let name = format!("{}{}", kept::KEPT_REF_PREFIX, k.id);
    let refs = git_out(&dir, &["for-each-ref", "--format=%(refname)", "refs/glowfly/"]);
    assert!(refs.lines().any(|l| l == name), "{refs}");
    // Reachable: a pruning gc keeps the blob.
    git(&dir, &["reflog", "expire", "--expire=now", "--all"]);
    git(&dir, &["gc", "-q", "--prune=now"]);
    assert_eq!(Repository::open(&dir).unwrap().find_blob(before.blob).unwrap().content(), b"edited\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_discard_that_changes_nothing_keeps_no_record() {
    let dir = base("kept-nothing");
    assert!(discard_saving(&dir, "f.txt", Discarded::File, || Ok(())).unwrap().is_none());
    assert!(kept::all_for_test(&dir).is_empty());
    let failed = discard_saving(&dir, "f.txt", Discarded::Hunk, || Err(crate::error::Error::Other("no".into())));
    assert!(failed.is_err());
    assert!(kept::all_for_test(&dir).is_empty(), "a failed discard that left the file keeps none");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_list_hides_a_file_that_holds_its_bytes_again() {
    let dir = base("kept-hidden");
    write(&dir, "f.txt", "edited\n");
    discard(&dir);
    write(&dir, "f.txt", "edited\n");
    assert!(kept::list(&dir).unwrap().is_empty(), "back as it was: nothing to restore");
    assert_eq!(kept::all_for_test(&dir).len(), 1, "the record stays until it is pruned");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_never_writes_over_anything() {
    let dir = base("kept-restore");
    write(&dir, "f.txt", "edited\n");
    discard(&dir);
    let id = kept::list(&dir).unwrap()[0].id;
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::Occupied { path: "f.txt".into() });
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\n", "untouched");

    assert_eq!(kept::restore(&dir, id, true).unwrap(), Restored::Wrote { path: "f (restored).txt".into() });
    assert_eq!(read(&dir, "f (restored).txt").unwrap(), b"edited\n");
    assert_eq!(kept::restore(&dir, id, true).unwrap(), Restored::Wrote { path: "f (restored 2).txt".into() });

    std::fs::remove_file(dir.join("f.txt")).unwrap();
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::Wrote { path: "f.txt".into() });
    assert_eq!(read(&dir, "f.txt").unwrap(), b"edited\n");
    assert_eq!(kept::restore(&dir, id, false).unwrap(), Restored::AlreadyThere { path: "f.txt".into() });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copy_names_keep_the_extension_and_folder() {
    let dir = base("kept-names");
    assert_eq!(kept::copy_name(&dir, "src/a.rs").unwrap(), "src/a (restored).rs");
    assert_eq!(kept::copy_name(&dir, ".env").unwrap(), ".env (restored)");
    assert_eq!(kept::copy_name(&dir, "Makefile").unwrap(), "Makefile (restored)");
    assert_eq!(kept::copy_name(&dir, "a.tar.gz").unwrap(), "a.tar (restored).gz");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_file_is_kept_and_comes_back_from_the_record() {
    let dir = base("kept-delete");
    write(&dir, "new.txt", "untracked work\n");
    let s = delete_saving(&dir, "new.txt", DeleteVia::Permanent, true, T).unwrap();
    assert!(s.undo.is_some());
    let list = kept::list(&dir).unwrap();
    let [k] = &list[..] else { panic!("{list:?}") };
    assert_eq!(k.what, KeptWhat::Delete);
    assert_eq!(kept::restore(&dir, k.id, false).unwrap(), Restored::Wrote { path: "new.txt".into() });
    assert_eq!(read(&dir, "new.txt").unwrap(), b"untracked work\n");
    assert!(kept::list(&dir).unwrap().is_empty());

    write(&dir, "new.txt", "more untracked work\n");
    let s = discard_untracked_saving(&dir, "new.txt", DeleteVia::Permanent, true, T).unwrap();
    assert!(s.undo.is_some());
    let whats: Vec<KeptWhat> = kept::list(&dir).unwrap().iter().map(|k| k.what).collect();
    assert!(whats.contains(&KeptWhat::Untracked) && whats.contains(&KeptWhat::Delete), "{whats:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dropped_stash_is_kept_until_it_is_back_in_the_list() {
    let dir = base("kept-stash");
    write(&dir, "f.txt", "stashed\n");
    git(&dir, &["stash", "push", "-q", "-m", "keep me"]);
    let stash = Repository::open(&dir).unwrap().refname_to_id("refs/stash").unwrap();
    let kind = UndoKind::classify(&args(&["stash", "drop"])).unwrap();
    let b = before(&dir, &kind).unwrap();
    assert!(kept::list(&dir).unwrap().is_empty(), "still in the stash list");
    git(&dir, &["stash", "drop", "-q"]);
    let a = after(&dir, kind, b).unwrap().expect("recorded");
    assert!(matches!(a, Action::StashDrop { stash: s, .. } if s == stash));
    let list = kept::list(&dir).unwrap();
    let [k] = &list[..] else { panic!("{list:?}") };
    assert_eq!(k.item, KeptItem::Stash { stash, message: "On main: keep me".into() });
    git(&dir, &["reflog", "expire", "--expire=now", "--all"]);
    git(&dir, &["gc", "-q", "--prune=now"]);
    assert!(Repository::open(&dir).unwrap().find_commit(stash).is_ok(), "reachable through the record");
    git(&dir, &["stash", "store", "-m", "On main: keep me", &stash.to_string()]);
    assert!(kept::list(&dir).unwrap().is_empty(), "back in the list");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_records_are_pruned_at_the_next_one() {
    let dir = base("kept-prune");
    let blob = Repository::open(&dir).unwrap().blob(b"old\n").unwrap();
    let saved = Saved { blob, executable: false };
    let day = 86_400;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let recent = kept::keep_file_at(&dir, "recent.txt", saved, now - (kept::KEEP_DAYS - 1) * day).unwrap();
    let old = kept::keep_file_at(&dir, "old.txt", saved, now - (kept::KEEP_DAYS + 1) * day).unwrap();
    let ids = |dir: &Path| kept::all_for_test(dir).into_iter().map(|k| k.id).collect::<Vec<_>>();
    assert!(ids(&dir).contains(&old), "pruning runs before a record is written");
    write(&dir, "f.txt", "edited\n");
    discard(&dir);
    let left = ids(&dir);
    assert!(!left.contains(&old) && left.contains(&recent) && left.len() == 2, "{left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
