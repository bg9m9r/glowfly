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

const BEFORE: FileAtOptions = FileAtOptions { before: true, smudge: false };

#[test]
fn before_reads_the_version_a_deleting_commit_removed() {
    let dir = scratch_repo("file-at-before");
    let parent = commit_bytes(&dir, "a.txt", b"one\ntwo\n", "first");
    git(&dir, &["rm", "-q", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove"]);
    let removed = head(&dir);
    let f = file_at_with(&dir, removed, "a.txt", BEFORE).unwrap();
    assert_eq!(f.lines, ["one", "two"]);
    assert_eq!(f.read_at, parent);
    assert_eq!(file_at(&dir, parent, "a.txt").unwrap().read_at, parent);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn before_on_a_merge_reads_the_parent_that_has_the_file() {
    let dir = scratch_repo("file-at-before-merge");
    commit_bytes(&dir, "base.txt", b"base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let side = commit_bytes(&dir, "f.txt", b"from side\n", "add f");
    git(&dir, &["checkout", "-q", "main"]);
    commit_bytes(&dir, "other.txt", b"o\n", "main edit");
    git(&dir, &["merge", "-q", "--no-ff", "--no-commit", "side"]);
    git(&dir, &["rm", "-q", "-f", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "merge without f"]);
    let merge = head(&dir);
    let f = file_at_with(&dir, merge, "f.txt", BEFORE).unwrap();
    assert_eq!(f.lines, ["from side"]);
    assert_eq!(f.read_at, side);
    match file_at_with(&dir, merge, "nope.txt", BEFORE) {
        Err(Error::NotFound(m)) => assert!(m.starts_with("nope.txt is not in the parents of "), "{m}"),
        other => panic!("expected NotFound, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn pointer_text(size: u64) -> String {
    format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n",
        "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393"
    )
}

#[test]
fn a_pointer_is_shown_as_stored_unless_smudging() {
    let dir = scratch_repo("file-at-pointer");
    let c = commit_bytes(&dir, "big.bin", pointer_text(9 * 1024 * 1024).as_bytes(), "pointer");
    let f = file_at(&dir, c, "big.bin").unwrap();
    assert_eq!(f.lfs, LfsState::Pointer);
    assert_eq!(f.lines[0], "version https://git-lfs.github.com/spec/v1");
    // Too large to show: refused before git is asked for the content.
    let smudge = FileAtOptions { before: false, smudge: true };
    match file_at_with(&dir, c, "big.bin", smudge) {
        Err(Error::Unsupported(m)) => assert_eq!(m, "big.bin is too large to show (9 MB)"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
    assert_eq!(file_at(&dir, c, "big.bin").unwrap().lfs, LfsState::Pointer);
    let plain = commit_bytes(&dir, "plain.txt", b"text\n", "plain");
    assert_eq!(file_at_with(&dir, plain, "plain.txt", smudge).unwrap().lfs, LfsState::None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn smudging_reads_an_lfs_file_s_content() {
    if crate::testutil::lfs_or_skip("smudging_reads_an_lfs_file_s_content").is_none() {
        return;
    }
    let dir = scratch_repo("file-at-lfs");
    git(&dir, &["lfs", "install", "--local"]);
    git(&dir, &["lfs", "track", "*.bin"]);
    let c = commit_bytes(&dir, "a.bin", b"first version\n", "lfs file");
    let f = file_at_with(&dir, c, "a.bin", FileAtOptions { before: false, smudge: true }).unwrap();
    assert_eq!(f.lines, ["first version"]);
    assert_eq!(f.lfs, LfsState::Content);
    let stored = file_at(&dir, c, "a.bin").unwrap();
    assert_eq!(stored.lfs, LfsState::Pointer);
    assert!(stored.lines[0].starts_with("version https://git-lfs"), "{:?}", stored.lines);
    let _ = std::fs::remove_dir_all(&dir);
}
