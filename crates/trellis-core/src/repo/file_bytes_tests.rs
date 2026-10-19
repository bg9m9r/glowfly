//! A file's bytes as of a commit (`file_bytes.rs`).

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

const STORED: FileAtOptions = FileAtOptions { before: false, smudge: false };
const SMUDGE: FileAtOptions = FileAtOptions { before: false, smudge: true };

/// A PNG signature followed by NULs: binary to git, whatever the decoder makes of it.
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01";

#[test]
fn binary_files_come_back_byte_for_byte() {
    let dir = scratch_repo("file-bytes-binary");
    let c = commit_bytes(&dir, "logo.png", PNG_BYTES, "logo");
    let f = file_bytes_at(&dir, c, "logo.png", STORED).unwrap();
    assert_eq!(f.bytes, PNG_BYTES);
    assert_eq!((f.lfs, f.read_at), (LfsState::None, c));
    // The text view still refuses it.
    assert!(matches!(file_at(&dir, c, "logo.png"), Err(Error::Unsupported(m)) if m == BINARY_NOT_SHOWN));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn before_reads_the_parent_of_a_deleting_commit() {
    let dir = scratch_repo("file-bytes-before");
    let parent = commit_bytes(&dir, "logo.png", PNG_BYTES, "logo");
    git(&dir, &["rm", "-q", "logo.png"]);
    git(&dir, &["commit", "-q", "-m", "remove"]);
    let f = file_bytes_at(&dir, head(&dir), "logo.png", FileAtOptions { before: true, smudge: false }).unwrap();
    assert_eq!((f.bytes.as_slice(), f.read_at), (PNG_BYTES, parent));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folders_submodules_and_missing_paths_are_errors() {
    let dir = scratch_repo("file-bytes-errors");
    let c = commit_bytes(&dir, "img/a.png", PNG_BYTES, "folder");
    let sub = "1234567890123456789012345678901234567890";
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{sub},vendor")]);
    git(&dir, &["commit", "-q", "-m", "submodule"]);
    let with_sub = head(&dir);
    let unsupported = |r: crate::error::Result<FileBytes>| match r {
        Err(Error::Unsupported(m)) => m,
        other => panic!("expected Unsupported, got {other:?}"),
    };
    assert_eq!(unsupported(file_bytes_at(&dir, c, "img", STORED)), "img is a folder");
    assert_eq!(unsupported(file_bytes_at(&dir, with_sub, "vendor", STORED)), "vendor is a submodule");
    match file_bytes_at(&dir, c, "nope.png", STORED) {
        Err(Error::NotFound(m)) => assert!(m.starts_with("nope.png is not in commit "), "{m}"),
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
fn pointers_come_back_as_stored_and_too_large_ones_are_refused() {
    let dir = scratch_repo("file-bytes-pointer");
    let pointer = pointer_text(9 * 1024 * 1024);
    let c = commit_bytes(&dir, "big.png", pointer.as_bytes(), "pointer");
    let f = file_bytes_at(&dir, c, "big.png", STORED).unwrap();
    assert_eq!(f.bytes, pointer.as_bytes());
    assert_eq!(f.lfs, LfsState::Pointer);
    match file_bytes_at(&dir, c, "big.png", SMUDGE) {
        Err(Error::Unsupported(m)) => assert_eq!(m, "big.png is too large to show (9 MB)"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn smudging_reads_an_lfs_image_s_content() {
    if lfs_or_skip("smudging_reads_an_lfs_image_s_content").is_none() {
        return;
    }
    let dir = scratch_repo("file-bytes-lfs");
    git(&dir, &["lfs", "install", "--local"]);
    git(&dir, &["lfs", "track", "*.png"]);
    let c = commit_bytes(&dir, "pic.png", PNG_BYTES, "lfs image");
    let f = file_bytes_at(&dir, c, "pic.png", SMUDGE).unwrap();
    assert_eq!(f.bytes, PNG_BYTES);
    assert_eq!(f.lfs, LfsState::Content);
    let stored = file_bytes_at(&dir, c, "pic.png", STORED).unwrap();
    assert_eq!(stored.lfs, LfsState::Pointer);
    assert!(stored.bytes.starts_with(b"version https://git-lfs"));
    let _ = std::fs::remove_dir_all(&dir);
}
