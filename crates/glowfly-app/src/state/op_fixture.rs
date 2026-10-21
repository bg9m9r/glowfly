//! Tests only: a loaded snapshot of a one-commit repository, for the pure rules of the
//! operation in progress to change the state and file rows of.

use glowfly_core::repo::{FileEntry, LoadOptions, Snapshot, load};
use std::path::Path;

/// A repository with a.txt committed, loaded; `name` keeps each test's folder apart.
pub fn snapshot(name: &str) -> Snapshot {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-op-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = git2::Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("T", "t@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "one", &tree, &[]).unwrap();
    load(&dir, &LoadOptions::all(10)).unwrap()
}

/// File rows from (path, status) pairs.
pub fn rows(list: &[(&str, char)]) -> Vec<FileEntry> {
    list.iter().map(|(p, s)| FileEntry::new(*p, *s)).collect()
}
