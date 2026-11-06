//! Sweeps of trees holding nested repositories whose files name a network place
//! (`testutil::net_place()`, never touched): walked as plain folders, never tracked, and dropped
//! when a tracked one comes to name one.

use super::tests::{later, moved, repo, start};
use super::*;
use crate::testutil::{git_spelling, link_to, net_place, write};

/// A new file in `inner`, found by a later sweep.
fn a_new_file_is_found(s: &mut Sweep, ig: &mut Ignores, inner: &Path) {
    write(inner, "b.txt", "b\n");
    moved(inner);
    let found = s.sweep(ig, later()).expect("the tree is still swept");
    assert!(found.contains(&inner.join("b.txt")), "{found:?}");
}

#[test]
fn a_nested_git_file_naming_a_network_place_is_walked_as_a_plain_folder() {
    let dir = repo("sweep-net-gitfile", 0);
    let inner = dir.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    write(&inner, ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("x/.git"))));
    write(&inner, "a.txt", "a\n");
    let (mut s, mut ig) = start(&dir);
    assert!(!s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")));
    assert!(s.seen.as_ref().unwrap().folders.contains_key(Path::new("inner")), "walked");
    assert_eq!(nested::refs_of(&inner, &dir), None);
    a_new_file_is_found(&mut s, &mut ig, &inner);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_nested_commondir_naming_a_network_place_is_not_followed() {
    let dir = repo("sweep-net-commondir", 0);
    let inner = dir.join("inner");
    let admin = dir.join("admin");
    std::fs::create_dir_all(admin.join("objects")).unwrap();
    std::fs::create_dir_all(&inner).unwrap();
    write(&admin, "HEAD", "ref: refs/heads/main\n");
    write(&admin, "commondir", &format!("{}\n", git_spelling(&net_place().join("repo/.git"))));
    write(&inner, ".git", "gitdir: ../admin\n");
    write(&inner, "a.txt", "a\n");
    let (mut s, mut ig) = start(&dir);
    assert!(!s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")));
    assert_eq!(nested::refs_of(&inner, &dir), None);
    a_new_file_is_found(&mut s, &mut ig, &inner);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tracked_nested_repository_rewritten_to_a_network_place_is_dropped() {
    let dir = repo("sweep-net-rewritten", 0);
    let inner = dir.join("inner");
    git2::Repository::init(&inner).unwrap();
    let (mut s, mut ig) = start(&dir);
    assert!(s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")));
    std::fs::remove_dir_all(inner.join(".git")).unwrap();
    write(&inner, ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("x/.git"))));
    let found = s.sweep(&mut ig, later()).unwrap();
    assert_eq!(found.iter().filter(|p| **p == inner.join(".git").join("HEAD")).count(), 1, "{found:?}");
    assert!(!s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")), "no longer tracked");
    let again = s.sweep(&mut ig, later()).unwrap();
    assert!(!again.contains(&inner.join(".git").join("HEAD")), "named once: {again:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tracked_nested_repository_whose_config_comes_to_name_a_network_place_is_dropped() {
    let dir = repo("sweep-net-config-later", 0);
    let inner = dir.join("inner");
    git2::Repository::init(&inner).unwrap();
    let (mut s, mut ig) = start(&dir);
    assert!(s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")));
    let config = inner.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    let excludes = git_spelling(&net_place().join("excludes"));
    std::fs::write(&config, format!("{text}[core]\n\texcludesFile = {excludes}\n")).unwrap();
    let found = s.sweep(&mut ig, later()).unwrap();
    assert_eq!(found.iter().filter(|p| **p == inner.join(".git").join("HEAD")).count(), 1, "{found:?}");
    assert!(!s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")), "its ignore rules are no longer asked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_nested_head_link_to_a_network_place_is_not_read() {
    let dir = repo("sweep-net-head-link", 0);
    let inner = dir.join("inner");
    git2::Repository::init(&inner).unwrap();
    assert!(nested::refs_of(&inner, &dir).is_some());
    let head = inner.join(".git/HEAD");
    std::fs::remove_file(&head).unwrap();
    if link_to(&net_place().join("HEAD"), &head, false) {
        assert_eq!(nested::refs_of(&inner, &dir), None);
        let (s, _) = start(&dir);
        assert!(!s.seen.as_ref().unwrap().nested.contains_key(Path::new("inner")), "nor tracked");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
