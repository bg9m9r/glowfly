//! Reverting a commit's or comparison's whole change to a file (`RevertPart::File`) against real
//! repositories: modified, added, deleted, binary and mode-only files, the merge with later
//! edits, and the current name after a rename.

use super::revert_tests::{EDITED, TEN, commit, edited, head, index_text, numbered, read, request, text};
use super::*;
use crate::error::{Error, Result};
use crate::testutil::*;
use std::path::Path;

fn revert_file(dir: &Path, target: &DiffTarget) -> Result<Reverted> {
    request(dir, target, RevertPart::File, None)
}

#[test]
fn file_revert_modified() {
    let dir = scratch_repo("revert-file-mod");
    commit(&dir, &[("f.txt", &numbered(30, &[]))], "base");
    let c = commit(&dir, &[("f.txt", &numbered(30, &[(2, "two"), (25, "twenty-five")]))], "two edits");
    assert_eq!(revert_file(&dir, &DiffTarget::Commit(c, "f.txt".into())).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[]));
    assert_eq!(index_text(&dir, "f.txt"), numbered(30, &[(2, "two"), (25, "twenty-five")]), "the index is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_merges_later_edits() {
    let dir = scratch_repo("revert-file-merge");
    commit(&dir, &[("f.txt", &numbered(30, &[]))], "base");
    let c = commit(&dir, &[("f.txt", &numbered(30, &[(2, "two"), (25, "twenty-five")]))], "two edits");
    write(&dir, "f.txt", &numbered(30, &[(2, "two"), (14, "mine"), (25, "twenty-five")]));
    assert_eq!(revert_file(&dir, &DiffTarget::Commit(c, "f.txt".into())).map_err(|e| e.to_string()), Ok(Reverted::Merged));
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[(14, "mine")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_conflicts() {
    let (dir, target) = edited("revert-file-conflict");
    write(&dir, "f.txt", &EDITED.replace("five", "FIVE"));
    assert_eq!(revert_file(&dir, &target).map_err(|e| e.to_string()), Ok(Reverted::Conflicts));
    let t = text(&dir, "f.txt");
    assert!(t.contains("<<<<<<< working copy") && t.contains("FIVE") && t.contains(">>>>>>> revert of"), "{t}");
    assert_eq!(index_text(&dir, "f.txt"), EDITED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_added_unchanged_removes() {
    let dir = scratch_repo("revert-file-added");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    let c = commit(&dir, &[("h.txt", "x\ny\n")], "add h");
    assert_eq!(revert_file(&dir, &DiffTarget::Commit(c, "h.txt".into())).map_err(|e| e.to_string()), Ok(Reverted::Removed));
    assert!(!dir.join("h.txt").exists());
    assert_eq!(index_text(&dir, "h.txt"), "x\ny\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_added_then_edited() {
    let dir = scratch_repo("revert-file-added-edited");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    let c = commit(&dir, &[("h.txt", "x\ny\n")], "add h");
    write(&dir, "h.txt", "x\nmine\ny\n");
    let r = revert_file(&dir, &DiffTarget::Commit(c, "h.txt".into()));
    let t = text(&dir, "h.txt");
    match r {
        Ok(Reverted::Conflicts) => assert!(t.contains("<<<<<<< working copy") && t.contains("mine"), "{t}"),
        Ok(Reverted::Merged) => assert_eq!(t, "mine\n"),
        other => panic!("{other:?}: {t}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_deleted_recreates() {
    let dir = scratch_repo("revert-file-deleted");
    write(&dir, "g.sh", "echo a\necho b\n");
    write(&dir, "keep.txt", "k\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["update-index", "--chmod=+x", "g.sh"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::remove_file(dir.join("g.sh")).unwrap();
    let c = commit(&dir, &[], "delete g");
    let target = DiffTarget::Commit(c, "g.sh".into());

    write(&dir, "g.sh", "other\n");
    let err = revert_file(&dir, &target).unwrap_err();
    assert!(err.to_string().contains("exists in the working tree"), "{err}");
    assert_eq!(read(&dir, "g.sh"), b"other\n");

    std::fs::remove_file(dir.join("g.sh")).unwrap();
    assert_eq!(revert_file(&dir, &target).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(read(&dir, "g.sh"), b"echo a\necho b\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("g.sh")).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "executable again: {mode:o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_between() {
    let dir = scratch_repo("revert-file-between");
    let a = commit(&dir, &[("f.txt", &numbered(10, &[]))], "A");
    commit(&dir, &[("f.txt", &numbered(10, &[(4, "four")]))], "B");
    let c = commit(&dir, &[("f.txt", &numbered(10, &[(4, "four"), (9, "nine")]))], "C");
    assert_eq!(revert_file(&dir, &DiffTarget::Between(a, c, "f.txt".into())).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), numbered(10, &[]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_mode_only() {
    let dir = scratch_repo("revert-file-mode");
    commit(&dir, &[("f.txt", TEN)], "base");
    git(&dir, &["update-index", "--chmod=+x", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "chmod"]);
    let c = head(&dir);
    let err = revert_file(&dir, &DiffTarget::Commit(c, "f.txt".into())).unwrap_err();
    assert!(err.to_string().contains("only the file mode changed"), "{err}");
    assert_eq!(text(&dir, "f.txt"), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_binary() {
    let dir = scratch_repo("revert-file-binary");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    std::fs::write(dir.join("b.bin"), b"\0\x01\x02").unwrap();
    let added = commit(&dir, &[], "add b");
    std::fs::write(dir.join("b.bin"), b"\0\x03\x04").unwrap();
    let modified = commit(&dir, &[], "change b");
    let r = revert_file(&dir, &DiffTarget::Commit(modified, "b.bin".into()));
    assert!(matches!(&r, Err(Error::Unsupported(e)) if e.contains("binary")), "{r:?}");
    assert_eq!(read(&dir, "b.bin"), b"\0\x03\x04");
    std::fs::write(dir.join("b.bin"), b"\0\x01\x02").unwrap();
    assert_eq!(revert_file(&dir, &DiffTarget::Commit(added, "b.bin".into())).map_err(|e| e.to_string()), Ok(Reverted::Removed));
    assert!(!dir.join("b.bin").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_refuses_a_binary_working_file() {
    let dir = scratch_repo("revert-file-nul-work");
    commit(&dir, &[("data.txt", &numbered(10, &[]))], "base");
    let c = commit(&dir, &[("data.txt", &numbered(10, &[(4, "four")]))], "edit");
    let mine = numbered(10, &[(4, "four"), (8, "ei\0ght")]);
    write(&dir, "data.txt", &mine);
    let r = revert_file(&dir, &DiffTarget::Commit(c, "data.txt".into()));
    assert!(matches!(&r, Err(Error::Unsupported(e)) if e.contains("binary")), "{r:?}");
    assert_eq!(text(&dir, "data.txt"), mine, "the working file is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_refuses_binary_commit_versions_diffed_as_text() {
    let dir = scratch_repo("revert-file-nul-commit");
    commit(&dir, &[(".gitattributes", "*.log diff\n"), ("a.log", "x\0\none\n")], "base");
    let c = commit(&dir, &[("a.log", "x\0\ntwo\n")], "edit");
    write(&dir, "a.log", "x\0\ntwo\nmine\n");
    let r = revert_file(&dir, &DiffTarget::Commit(c, "a.log".into()));
    assert!(matches!(&r, Err(Error::Unsupported(e)) if e.contains("binary")), "{r:?}");
    assert_eq!(text(&dir, "a.log"), "x\0\ntwo\nmine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_refuses_converted() {
    let dir = scratch_repo("revert-file-lfs");
    commit(&dir, &[(".gitattributes", "*.dat filter=lfs\n"), ("d.dat", "one\n")], "base");
    let c = commit(&dir, &[("d.dat", "two\n")], "edit");
    let r = revert_file(&dir, &DiffTarget::Commit(c, "d.dat".into()));
    assert!(matches!(r, Err(Error::Unsupported(_))), "{r:?}");
    assert_eq!(text(&dir, "d.dat"), "two\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_revert_with_dest() {
    let dir = scratch_repo("revert-file-dest");
    commit(&dir, &[("old.txt", &numbered(20, &[]))], "base");
    let c = commit(&dir, &[("old.txt", &numbered(20, &[(3, "three"), (17, "seventeen")]))], "edit");
    std::fs::rename(dir.join("old.txt"), dir.join("new.txt")).unwrap();
    commit(&dir, &[], "rename");
    let target = DiffTarget::Commit(c, "old.txt".into());
    assert_eq!(request(&dir, &target, RevertPart::File, Some("new.txt")).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "new.txt"), numbered(20, &[]));
    assert!(!dir.join("old.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
