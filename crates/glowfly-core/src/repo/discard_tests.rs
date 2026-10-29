//! Discarding an untracked file against real repositories, with the stand-in trashes from
//! `removal_tests`: it goes to the trash (or for good) and its emptied folders are pruned, a
//! refusal changes nothing, a path that is tracked or staged by now is refused, and links never
//! reach the trash.

use super::removal_tests::{fake_bin, in_bin, never_called, no_bin};
use super::workfile_tests::{commit, status, text};
use super::*;
use crate::error::Error;
use crate::testutil::*;

#[test]
fn discard_untracked_to_trash() {
    let dir = scratch_repo("dsc-trash");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    write(&dir, "sub/deep/new.txt", "discard me dsc-trash\n");

    assert_eq!(discard_untracked(&dir, "sub/deep/new.txt", Removal::Trash(fake_bin)).unwrap(), Deleted::Trashed);
    assert!(!dir.join("sub/deep/new.txt").exists());
    assert!(in_bin("new.txt", "discard me dsc-trash\n"));
    assert!(!dir.join("sub").exists(), "emptied folders are pruned");
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_untracked_permanent() {
    let dir = scratch_repo("dsc-perm");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    write(&dir, "new.txt", "gone for good dsc-perm\n");

    assert_eq!(discard_untracked(&dir, "new.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(!dir.join("new.txt").exists());
    assert!(!in_bin("new.txt", "gone for good dsc-perm\n"));
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_untracked_trash_refusal_changes_nothing() {
    let dir = scratch_repo("dsc-refuse");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    write(&dir, "sub/deep/new.txt", "stay\n");
    let before = status(&dir);

    match discard_untracked(&dir, "sub/deep/new.txt", Removal::Trash(no_bin)) {
        Err(Error::TrashRefused(r)) => assert!(r.detail.contains("no bin here"), "{r}"),
        other => panic!("expected TrashRefused, got {other:?}"),
    }
    assert_eq!(text(&dir, "sub/deep/new.txt"), "stay\n");
    assert!(dir.join("sub/deep").is_dir());
    assert_eq!(status(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_untracked_refuses_tracked_and_staged() {
    let dir = scratch_repo("dsc-tracked");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "a.txt", "a edited\n");
    write(&dir, "n.txt", "staged new\n");
    git(&dir, &["add", "n.txt"]);
    let before = status(&dir);

    for (path, contents) in [("a.txt", "a edited\n"), ("n.txt", "staged new\n")] {
        match discard_untracked(&dir, path, Removal::Trash(never_called)) {
            Err(Error::Other(m)) => assert!(m.contains("tracked"), "{m}"),
            other => panic!("expected Other for {path}, got {other:?}"),
        }
        assert_eq!(text(&dir, path), contents);
    }
    assert_eq!(status(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_untracked_refuses_escape_and_folder() {
    let dir = scratch_repo("dsc-escape");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    let name = format!("dsc-escape-outside-{}.txt", std::process::id());
    let outside = dir.parent().unwrap().join(&name);
    std::fs::write(&outside, "outside\n").unwrap();

    assert!(discard_untracked(&dir, &format!("../{name}"), Removal::Permanent).is_err());
    assert!(discard_untracked(&dir, ".git/config", Removal::Permanent).is_err());
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside\n");
    assert!(dir.join(".git/config").exists());

    std::fs::create_dir_all(dir.join("folder")).unwrap();
    write(&dir, "folder/x.txt", "x\n");
    assert!(matches!(discard_untracked(&dir, "folder", Removal::Permanent), Err(Error::Unsupported(_))));
    assert_eq!(text(&dir, "folder/x.txt"), "x\n");

    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_untracked_link() {
    let dir = scratch_repo("dsc-link");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    let target = dir.parent().unwrap().join(format!("dsc-link-target-{}.txt", std::process::id()));
    std::fs::write(&target, "target\n").unwrap();
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&target, dir.join("link")).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&target, dir.join("link")).is_ok();
    if made {
        assert_eq!(discard_untracked(&dir, "link", Removal::Trash(never_called)).unwrap(), Deleted::LinkRemoved);
        assert!(dir.join("link").symlink_metadata().is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target\n");
    }
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The unstaged diff's only hunk of `path`, as the diff viewer shows it.
fn shown(dir: &std::path::Path, path: &str) -> Hunk {
    load_diff(dir, &DiffTarget::Unstaged(path.into())).unwrap().hunks.remove(0)
}

#[test]
fn whole_discard_deletes_an_untracked_file_only_as_shown() {
    let dir = scratch_repo("dsc-whole");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    write(&dir, "new.txt", "a\nb");
    let hunk = shown(&dir, "new.txt");
    assert_eq!(whole_discard(&dir, "new.txt", &hunk, false).unwrap(), WholeDiscard::Delete);
    assert_eq!(whole_discard(&dir, "new.txt", &hunk, true).unwrap(), WholeDiscard::Delete, "whitespace hidden");

    // Changed since the diff was shown: never deleted.
    write(&dir, "new.txt", "a\nb\nc\n");
    assert!(matches!(whole_discard(&dir, "new.txt", &hunk, false), Err(Error::NotFound(_))));
    std::fs::remove_file(dir.join("new.txt")).unwrap();
    assert!(matches!(whole_discard(&dir, "new.txt", &hunk, false), Err(Error::NotFound(_))), "gone");

    // Staged by now: refused.
    write(&dir, "new.txt", "a\nb");
    git(&dir, &["add", "new.txt"]);
    assert!(whole_discard(&dir, "new.txt", &hunk, false).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_discard_refuses_a_hunk_with_unchanged_lines() {
    let dir = scratch_repo("dsc-whole-edit");
    commit(&dir, &[("f.txt", "a\n")], "one");
    write(&dir, "f.txt", "a\nb\n");
    let hunk = shown(&dir, "f.txt");
    assert!(matches!(whole_discard(&dir, "f.txt", &hunk, false), Err(Error::NotFound(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_discard_of_an_intent_to_add_file_discards_lines() {
    let dir = scratch_repo("dsc-whole-ita");
    commit(&dir, &[("keep.txt", "k\n")], "one");
    write(&dir, "ita.txt", "x\ny\n");
    git(&dir, &["add", "-N", "ita.txt"]);
    let (hunk, lines) = super::lines_tests::pick(&dir, &DiffTarget::Unstaged("ita.txt".into()), &[('+', "x"), ('+', "y")]);
    assert_eq!(whole_discard(&dir, "ita.txt", &hunk, false).unwrap(), WholeDiscard::Lines);
    apply_lines(&dir, "ita.txt", 0, &hunk, &lines, HunkOp::Discard).unwrap();
    assert_eq!(std::fs::read(dir.join("ita.txt")).unwrap(), b"", "emptied, not deleted");
    let _ = std::fs::remove_dir_all(&dir);
}
