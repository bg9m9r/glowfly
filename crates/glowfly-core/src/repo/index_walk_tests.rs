//! The index walk against libgit2's own parse of real index files.

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

/// libgit2's (mode, path) list of the index file, sorted.
fn libgit2_entries(file: &Path) -> Vec<(u32, Vec<u8>)> {
    let mut out: Vec<(u32, Vec<u8>)> = git2::Index::open(file).unwrap().iter().map(|e| (e.mode, e.path)).collect();
    out.sort();
    out
}

fn walked(file: &Path) -> Option<Vec<(u32, Vec<u8>)>> {
    let mut out = entries_of_file(file, |_, _| true)?;
    out.sort();
    Some(out)
}

fn index_of(dir: &Path) -> PathBuf {
    dir.join(".git").join("index")
}

fn put(dir: &Path, file: &str, text: &str) {
    std::fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
    write(dir, file, text);
}

/// A committed repository with files in folders that share long prefixes.
fn tree(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    for folder in ["src/some/long/shared/prefix", "src/some/long/shared/other", "src/some/different", "docs"] {
        for i in 0..4 {
            put(&dir, &format!("{folder}/file-{i}.txt"), &format!("{folder} {i}\n"));
        }
    }
    put(&dir, "top.txt", "top\n");
    put(&dir, "caf\u{e9}/menu.txt", "menu\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

fn head(dir: &Path) -> String {
    let (ok, out) = crate::gitcli::run(dir, &crate::gitcli::args(&["rev-parse", "HEAD"]));
    assert!(ok);
    out.trim().to_owned()
}

#[test]
fn a_version_2_index_is_walked_as_libgit2_reads_it() {
    let dir = tree("walk-v2");
    git(&dir, &["update-index", "--index-version", "2"]);
    let found = walked(&index_of(&dir)).expect("walked");
    assert_eq!(found.len(), 18);
    assert_eq!(found, libgit2_entries(&index_of(&dir)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_version_3_index_from_intent_to_add_is_walked() {
    let dir = tree("walk-v3");
    put(&dir, "ita.txt", "i\n");
    git(&dir, &["add", "-N", "ita.txt"]);
    let bytes = std::fs::read(index_of(&dir)).unwrap();
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 3);
    assert_eq!(walked(&index_of(&dir)).expect("walked"), libgit2_entries(&index_of(&dir)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_version_4_index_with_shared_prefixes_is_walked() {
    let dir = tree("walk-v4");
    git(&dir, &["update-index", "--index-version", "4"]);
    let bytes = std::fs::read(index_of(&dir)).unwrap();
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 4);
    let found = walked(&index_of(&dir)).expect("walked");
    assert!(found.iter().any(|(_, p)| p == "src/some/long/shared/prefix/file-3.txt".as_bytes()));
    assert_eq!(found, libgit2_entries(&index_of(&dir)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn links_and_gitlinks_keep_their_modes_and_a_conflicted_gitlink_its_stages() {
    let dir = tree("walk-links");
    let blob = {
        let (ok, out) = crate::gitcli::run_with_input(&dir, &crate::gitcli::args(&["hash-object", "-w", "--stdin"]), "target\n", &[]);
        assert!(ok);
        out.trim().to_owned()
    };
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("120000,{blob},a-link")]);
    let commit = head(&dir);
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{commit},lib")]);
    let stages = format!("160000 {commit} 1\tfight\n160000 {commit} 2\tfight\n160000 {commit} 3\tfight\n");
    let (ok, out) = crate::gitcli::run_with_input(&dir, &crate::gitcli::args(&["update-index", "--index-info"]), &stages, &[]);
    assert!(ok, "{out}");
    for version in ["2", "4"] {
        git(&dir, &["update-index", "--index-version", version]);
        let file = index_of(&dir);
        let found = walked(&file).expect("walked");
        assert_eq!(found, libgit2_entries(&file), "version {version}");
        assert!(found.contains(&(0o120000, b"a-link".to_vec())));
        assert!(found.contains(&(0o160000, b"lib".to_vec())));
        assert_eq!(found.iter().filter(|(m, p)| *m == 0o160000 && p == b"fight").count(), 3);
        let only_links = entries_of_file(&file, |mode, _| mode == 0o120000).unwrap();
        assert_eq!(only_links, vec![(0o120000, b"a-link".to_vec())]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_split_index_is_not_walked() {
    let dir = tree("walk-split");
    git(&dir, &["update-index", "--split-index"]);
    put(&dir, "later.txt", "later\n");
    git(&dir, &["add", "later.txt"]);
    let found = crate::repo::index_ext::index_extensions(&index_of(&dir)).expect("an index the extension reader knows");
    assert!(found.iter().any(|e| e == "link"), "{found:?}");
    assert_eq!(entries_of_file(&index_of(&dir), |_, _| true), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sparse_index_is_not_walked() {
    let dir = tree("walk-sparse");
    git(&dir, &["sparse-checkout", "init", "--cone", "--sparse-index"]);
    git(&dir, &["sparse-checkout", "set", "docs"]);
    let found = crate::repo::index_ext::index_extensions(&index_of(&dir)).unwrap_or_default();
    if !found.iter().any(|e| e == "sdir") {
        // A git too old to write a sparse index.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    assert_eq!(entries_of_file(&index_of(&dir), |_, _| true), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_or_cut_file_is_not_walked() {
    let dir = tree("walk-bad");
    let bytes = std::fs::read(index_of(&dir)).unwrap();
    assert!(entries(&bytes, |_, _| true).is_some());
    assert_eq!(entries(&bytes[..bytes.len() - 30], |_, _| true), None, "cut inside the entries or the checksum");
    assert_eq!(entries(&bytes[..11], |_, _| true), None);
    assert_eq!(entries(b"", |_, _| true), None);
    let mut wrong = bytes.clone();
    wrong[0] = b'X';
    assert_eq!(entries(&wrong, |_, _| true), None, "not DIRC");
    let mut future = bytes.clone();
    future[7] = 9;
    assert_eq!(entries(&future, |_, _| true), None, "an unknown version");
    let _ = std::fs::remove_dir_all(&dir);
}
