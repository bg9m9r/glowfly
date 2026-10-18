//! A file's text as of a commit (`file_at.rs`).

use super::*;
use crate::error::Error;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit_bytes(dir: &Path, file: &str, bytes: &[u8], message: &str) -> Oid {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

#[test]
fn file_at_reads_the_version_in_that_commit() {
    let dir = scratch_repo("file-at-version");
    let old = commit_bytes(&dir, "a.txt", b"one\ntwo\n", "first");
    commit_bytes(&dir, "a.txt", b"three\n", "second");
    let f = file_at(&dir, old, "a.txt").unwrap();
    assert_eq!(f.lines, ["one", "two"]);
    assert_eq!(f.language, None);
    assert_eq!(file_at(&dir, head(&dir), "a.txt").unwrap().lines, ["three"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn crlf_and_final_newline_are_trimmed() {
    let dir = scratch_repo("file-at-crlf");
    let c = commit_bytes(&dir, "a.txt", b"a\r\nb\n", "crlf");
    assert_eq!(file_at(&dir, c, "a.txt").unwrap().lines, ["a", "b"]);
    let c = commit_bytes(&dir, "b.txt", b"x\n\ny", "no final newline");
    assert_eq!(file_at(&dir, c, "b.txt").unwrap().lines, ["x", "", "y"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_file_has_no_lines() {
    let dir = scratch_repo("file-at-empty");
    let c = commit_bytes(&dir, "empty.txt", b"", "empty");
    assert!(file_at(&dir, c, "empty.txt").unwrap().lines.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn binary_folder_submodule_and_missing_are_errors() {
    let dir = scratch_repo("file-at-errors");
    commit_bytes(&dir, "bin.dat", b"\x00\x01\x02binary", "binary");
    let c = commit_bytes(&dir, "src/a.rs", b"fn main() {}\n", "folder");
    // A gitlink entry, as a submodule leaves in the tree (no clone needed).
    let sub = "1234567890123456789012345678901234567890";
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{sub},vendor")]);
    git(&dir, &["commit", "-q", "-m", "submodule"]);
    let with_sub = head(&dir);

    let unsupported = |r: crate::error::Result<FileAt>| match r {
        Err(Error::Unsupported(m)) => m,
        other => panic!("expected Unsupported, got {other:?}"),
    };
    assert_eq!(unsupported(file_at(&dir, c, "bin.dat")), "Binary file not shown");
    assert_eq!(unsupported(file_at(&dir, c, "src")), "src is a folder");
    assert_eq!(unsupported(file_at(&dir, with_sub, "vendor")), "vendor is a submodule");
    match file_at(&dir, c, "nope.txt") {
        Err(Error::NotFound(m)) => assert!(m.starts_with("nope.txt is not in commit "), "{m}"),
        other => panic!("expected NotFound, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_utf8_is_read_lossily() {
    let dir = scratch_repo("file-at-lossy");
    let c = commit_bytes(&dir, "latin1.txt", b"caf\xe9\nok\n", "latin-1");
    assert_eq!(file_at(&dir, c, "latin1.txt").unwrap().lines, ["caf\u{FFFD}", "ok"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn linguist_language_attribute_is_reported() {
    let dir = scratch_repo("file-at-linguist");
    commit_bytes(&dir, ".gitattributes", b"*.conf linguist-language=TOML\n", "attributes");
    let c = commit_bytes(&dir, "app.conf", b"a = 1\n", "conf");
    assert_eq!(file_at(&dir, c, "app.conf").unwrap().language.as_deref(), Some("TOML"));
    let _ = std::fs::remove_dir_all(&dir);
}
