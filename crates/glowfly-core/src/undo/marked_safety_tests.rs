//! The safety checks around conflict changes and their undo: a file that can't be kept is never
//! written over, a change that fails partway still records what it changed, an undo stops at a
//! file or index that moved under it, at another operation with the same HEAD, at a blob that
//! is gone, and keeps a stage-0 entry's flags; paths that fold to one name, or an index with a
//! stage-0 entry beside a conflict, are refused.

use super::marked::conflicts_saving_with;
use super::marked_tests::{apply, binary, bytes, ids, mark, refusal, stages, text};
use super::*;
use crate::gitcli::run;
use crate::repo::{Side, mark_resolved, resolve_with_side};
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn failed(r: std::result::Result<Option<Action>, ConflictsFailed>) -> ConflictsFailed {
    r.expect_err("expected the change to fail")
}

#[test]
fn a_file_that_cant_be_kept_is_never_written_over() {
    let dir = binary("mks-too-large");
    // Bytes the object database doesn't hold already (as a stage would).
    std::fs::write(dir.join("logo.bin"), b"\0edited by hand\0").unwrap();
    let before = (stages(&dir, "logo.bin"), bytes(&dir, "logo.bin"));
    let paths = ["logo.bin".to_owned()];
    let expected = ids(&dir, "logo.bin");
    let mut ran = false;
    let f = failed(conflicts_saving_with(&dir, ConflictChange::Took(Side::Theirs), &paths, 2, || {
        ran = true;
        resolve_with_side(&dir, "logo.bin", Side::Theirs, &expected)
    }));
    assert!(!ran, "refused before it ran");
    assert!(f.undo.is_none() && f.error.to_string().contains("can't keep a copy of it"), "{}", f.error);
    assert_eq!((stages(&dir, "logo.bin"), bytes(&dir, "logo.bin")), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_that_fails_partway_records_what_it_changed() {
    let dir = scratch_repo("mks-partway");
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "base\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "theirs\n");
    }
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    for f in ["a.txt", "b.txt"] {
        write(&dir, f, "ours\n");
    }
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0);
    let conflicted = stages(&dir, "a.txt");
    let paths = ["a.txt".to_owned(), "b.txt".to_owned()];
    let f = failed(conflicts_saving_with(&dir, ConflictChange::MarkResolved, &paths, MAX_KEPT_BYTES, || {
        mark_resolved(&dir, &paths[..1])?;
        Err(crate::error::Error::Other("the second batch failed".into()))
    }));
    assert_eq!(f.error.to_string(), "the second batch failed");
    let undo = f.undo.expect("the first file's change is undoable");
    let Action::Conflicts { files, .. } = &undo else { panic!("{undo:?}") };
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a.txt"]);
    apply(&dir, &undo, false);
    assert_eq!(stages(&dir, "a.txt"), conflicted);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_changed_after_the_plan_is_left_alone() {
    let dir = binary("mks-raced");
    let a = mark(&dir, ConflictChange::Took(Side::Theirs), &["logo.bin"]);
    assert!(matches!(plan(&dir, &a, false).unwrap(), Plan::PutConflicts));
    write(&dir, "logo.bin", "written meanwhile\n");
    let e = put_conflicts(&dir, &a, false).unwrap_err().to_string();
    assert!(e.contains("logo.bin changed while it was being put back, so it was left as it is"), "{e}");
    assert_eq!(bytes(&dir, "logo.bin").as_deref(), Some(&b"written meanwhile\n"[..]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_index_changed_after_the_plan_is_left_alone() {
    let dir = text("mks-index-raced");
    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    assert!(matches!(plan(&dir, &a, false).unwrap(), Plan::PutConflicts));
    write(&dir, "a.txt", "staged meanwhile\n");
    git(&dir, &["add", "a.txt"]);
    let staged = stages(&dir, "a.txt");
    let e = put_conflicts(&dir, &a, false).unwrap_err().to_string();
    assert!(e.contains("a.txt changed in the index while it was being put back"), "{e}");
    assert_eq!(stages(&dir, "a.txt"), staged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_merge_with_the_same_head_refuses() {
    let dir = text("mks-other-merge");
    let a = mark(&dir, ConflictChange::MarkResolved, &["a.txt"]);
    let head = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    std::fs::write(dir.join(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    let m = refusal(&dir, &a, false);
    assert!(m.contains("The merge that was in progress during the marking a.txt resolved is over"), "{m}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_blob_lost_to_gc_refuses_before_anything_is_written() {
    let dir = binary("mks-gc");
    let a = mark(&dir, ConflictChange::Took(Side::Theirs), &["logo.bin"]);
    let Action::Conflicts { files, .. } = &a else { panic!() };
    let WorkAt::File(ours) = files[0].before.work else { panic!("{:?}", files[0].before.work) };
    let hex = ours.blob.to_string();
    std::fs::remove_file(dir.join(".git/objects").join(&hex[..2]).join(&hex[2..])).unwrap();
    let after = (stages(&dir, "logo.bin"), bytes(&dir, "logo.bin"));
    let m = refusal(&dir, &a, false);
    assert!(m.contains("Glowfly no longer has the index entries of logo.bin"), "{m}");
    assert_eq!((stages(&dir, "logo.bin"), bytes(&dir, "logo.bin")), after);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restored_entry_keeps_its_flags() {
    let dir = text("mks-flags");
    write(&dir, "a.txt", "one\nresolved\n");
    mark_resolved(&dir, &["a.txt".to_owned()]).unwrap();
    git(&dir, &["update-index", "--assume-unchanged", "a.txt"]);
    let flagged = |dir: &Path| run(dir, &args(&["ls-files", "-v", "--", "a.txt"])).1;
    assert!(flagged(&dir).starts_with('h'), "{}", flagged(&dir));
    let a = mark(&dir, ConflictChange::MarkConflicted, &["a.txt"]);
    apply(&dir, &a, false);
    assert!(flagged(&dir).starts_with('h'), "assume-unchanged is back: {}", flagged(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paths_folding_to_one_name_and_mixed_indexes_are_refused() {
    let dir = text("mks-case");
    git(&dir, &["config", "core.ignoreCase", "true"]);
    let paths = ["a.txt".to_owned(), "A.txt".to_owned()];
    let f = failed(conflicts_saving(&dir, ConflictChange::MarkResolved, &paths, || panic!("never runs")));
    assert!(f.error.to_string().contains("differs from another conflicted path only in case"), "{}", f.error);
    let _ = std::fs::remove_dir_all(&dir);

    let dir = text("mks-mixed");
    {
        let repo = Repository::open(&dir).unwrap();
        let mut index = repo.index().unwrap();
        let mut e = index.get_path(Path::new("a.txt"), 2).unwrap();
        e.flags &= !0x3000;
        index.add(&e).unwrap();
        index.write().unwrap();
    }
    let all = stages(&dir, "a.txt");
    if all.lines().count() == 4 {
        let paths = ["a.txt".to_owned()];
        let f = failed(conflicts_saving(&dir, ConflictChange::MarkResolved, &paths, || panic!("never runs")));
        assert!(f.error.to_string().contains("has a resolved entry beside its conflict"), "{}", f.error);
    } else {
        eprintln!("skipped: libgit2 didn't keep a stage-0 entry beside the conflict: {all}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
