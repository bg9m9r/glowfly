//! Submodule status against real repositories: in sync, dirty, out of sync, both, never
//! initialized, deinitialized, folder deleted, a gitlink without .gitmodules, a staged pointer
//! change and a .gitmodules that can't be parsed.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn subs(parent: &Path) -> Vec<SubmoduleInfo> {
    load(parent, &LoadOptions::all(100)).unwrap().submodules
}

fn only(parent: &Path) -> SubmoduleInfo {
    let list = subs(parent);
    assert_eq!(list.len(), 1, "{list:?}");
    list.into_iter().next().unwrap()
}

/// Removes a `with_submodule` fixture: the parent and its sibling library repository.
fn cleanup(parent: &Path) {
    let _ = std::fs::remove_dir_all(lib_dir(parent));
    let _ = std::fs::remove_dir_all(parent);
}

/// The fixture names the sibling `trellis-test-<name>-lib-<pid>`; the parent is
/// `trellis-test-<name>-<pid>`.
fn lib_stem(parent: &Path) -> String {
    let file = parent.file_name().unwrap().to_string_lossy().into_owned();
    let pid = format!("-{}", std::process::id());
    file.strip_suffix(&pid).unwrap_or(&file).to_owned()
}

fn lib_dir(parent: &Path) -> std::path::PathBuf {
    parent.with_file_name(format!("{}-lib-{}", lib_stem(parent), std::process::id()))
}

#[test]
fn lists_in_sync_submodule() {
    let (parent, _sub) = with_submodule("sm-in-sync");
    let lib = lib_dir(&parent);
    let sm = only(&parent);
    assert_eq!(sm.name, "libs/lib");
    assert_eq!(sm.path, "libs/lib");
    assert_eq!(sm.url.as_deref(), Some(format!("../{}", lib.file_name().unwrap().to_string_lossy()).as_str()));
    assert_eq!(sm.recorded, head(&lib));
    assert_eq!(sm.checked_out, Some(sm.recorded));
    assert_eq!(sm.state, SubmoduleState::InSync);
    assert!(sm.is_initialized() && !sm.is_dirty() && !sm.is_out_of_sync());
    assert_eq!(submodules_at(&parent).unwrap(), vec![sm]);
    cleanup(&parent);
}

#[test]
fn dirty_submodule() {
    let (parent, sub) = with_submodule("sm-dirty");
    write(&sub, "new.txt", "untracked\n");
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::Dirty);
    assert!(sm.is_dirty() && !sm.is_out_of_sync());

    std::fs::remove_file(sub.join("new.txt")).unwrap();
    write(&sub, "lib.txt", "changed\n");
    assert_eq!(only(&parent).state, SubmoduleState::Dirty, "a modified tracked file");

    git(&sub, &["add", "lib.txt"]);
    assert_eq!(only(&parent).state, SubmoduleState::Dirty, "a staged change inside");
    cleanup(&parent);
}

#[test]
fn out_of_sync_submodule() {
    let (parent, sub) = with_submodule("sm-out");
    let recorded = only(&parent).recorded;
    write(&sub, "lib.txt", "two\n");
    git(&sub, &["commit", "-q", "-am", "inside"]);
    let new = head(&sub);
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::OutOfSync);
    assert_eq!((sm.checked_out, sm.recorded), (Some(new), recorded));
    assert!(sm.is_out_of_sync() && !sm.is_dirty());

    write(&sub, "extra.txt", "untracked\n");
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::DirtyOutOfSync);
    assert!(sm.is_out_of_sync() && sm.is_dirty());

    // A staged pointer change: the index records the new commit.
    std::fs::remove_file(sub.join("extra.txt")).unwrap();
    git(&parent, &["add", "libs/lib"]);
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::InSync);
    assert_eq!(sm.recorded, new);
    cleanup(&parent);
}

#[test]
fn uninitialized_submodule() {
    let (parent, sub) = with_submodule("sm-uninit");
    git(&parent, &["submodule", "deinit", "-q", "-f", "libs/lib"]);
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::Uninitialized);
    assert_eq!(sm.checked_out, None);
    assert!(!sm.is_initialized());

    // .git/modules keeps the repository, so no clone (and no file transport) is needed.
    git(&parent, &["submodule", "update", "-q", "--init", "--", "libs/lib"]);
    assert_eq!(only(&parent).state, SubmoduleState::InSync);

    std::fs::remove_dir_all(&sub).unwrap();
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::Uninitialized, "folder deleted");
    assert_eq!(sm.checked_out, None);
    cleanup(&parent);
}

#[test]
fn gitlink_without_gitmodules_has_no_url() {
    let parent = scratch_repo("sm-bare-link");
    write(&parent, "a.txt", "one\n");
    git(&parent, &["add", "."]);
    git(&parent, &["commit", "-q", "-m", "first"]);
    assert!(subs(&parent).is_empty(), "a repository without submodules");

    let nested = parent.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);
    git(&nested, &["config", "user.name", "Test"]);
    git(&nested, &["config", "user.email", "test@example.com"]);
    git(&nested, &["config", "commit.gpgsign", "false"]);
    write(&nested, "n.txt", "n\n");
    git(&nested, &["add", "."]);
    git(&nested, &["commit", "-q", "-m", "nested"]);
    git(&parent, &["add", "nested"]);
    let sm = only(&parent);
    assert_eq!((sm.name.as_str(), sm.path.as_str()), ("nested", "nested"));
    assert_eq!(sm.url, None);
    assert_eq!(sm.recorded, head(&nested));
    assert_eq!(sm.state, SubmoduleState::InSync);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn broken_gitmodules_does_not_fail_load() {
    let (parent, _sub) = with_submodule("sm-broken");
    write(&parent, ".gitmodules", "[submodule\n");
    let snap = load(&parent, &LoadOptions::all(100)).expect("the repository still loads");
    // The list may be empty or partial; it just must not fail.
    assert!(snap.submodules.len() <= 1);
    assert!(status_at(&parent).is_ok(), "status reads the files without the submodules");
    cleanup(&parent);
}
