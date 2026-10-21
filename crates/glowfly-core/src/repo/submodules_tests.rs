//! Submodule status against real repositories: in sync, dirty, out of sync, both, never
//! initialized, deinitialized, folder deleted, added and not committed, a gitlink without
//! .gitmodules, a staged pointer change, a .gitmodules that can't be parsed, and the branch and
//! clone URL.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

#[test]
fn status_reload_reads_submodules_only_when_there_are_any() {
    let plain = scratch_repo("sm-if-any-plain");
    assert_eq!(submodules_if_any(&plain, false).unwrap(), None);
    assert_eq!(submodules_if_any(&plain, true).unwrap(), Some(Vec::new()));
    let _ = std::fs::remove_dir_all(&plain);

    let (parent, _sub) = with_submodule("sm-if-any");
    let list = submodules_if_any(&parent, false).unwrap().expect("a .gitmodules is there");
    assert_eq!(list, subs(&parent));
    cleanup(&parent);
}

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

/// The fixture names the sibling `glowfly-test-<name>-lib-<pid>`; the parent is
/// `glowfly-test-<name>-<pid>`.
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
    assert!(sm.left_behind.is_empty(), "the commit is on the submodule's main branch");

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
fn detached_commits_are_left_behind() {
    let (parent, sub) = with_submodule("sm-left-behind");
    // As `git submodule update` leaves it: HEAD detached at the recorded commit.
    git(&sub, &["checkout", "-q", "--detach"]);
    assert!(only(&parent).left_behind.is_empty(), "in sync");
    write(&sub, "lib.txt", "two\n");
    git(&sub, &["commit", "-q", "-am", "first inside"]);
    let first = head(&sub);
    write(&sub, "lib.txt", "three\n");
    git(&sub, &["commit", "-q", "-am", "second inside"]);
    let second = head(&sub);
    let sm = only(&parent);
    assert_eq!(sm.state, SubmoduleState::OutOfSync);
    assert_eq!(sm.left_behind, vec![(second, "second inside".to_owned()), (first, "first inside".to_owned())]);

    git(&sub, &["tag", "keep-first", &first.to_string()]);
    assert_eq!(only(&parent).left_behind, vec![(second, "second inside".to_owned())], "a tag keeps the first");
    git(&sub, &["branch", "keep"]);
    assert!(only(&parent).left_behind.is_empty(), "a branch keeps both");
    git(&sub, &["branch", "-q", "-D", "keep"]);
    git(&sub, &["tag", "-d", "keep-first"]);

    // Recording the second, then checking out the first: the recorded commit still reaches it.
    git(&parent, &["add", "libs/lib"]);
    git(&sub, &["checkout", "-q", &first.to_string()]);
    let sm = only(&parent);
    assert_eq!((sm.state, sm.recorded), (SubmoduleState::OutOfSync, second));
    assert!(sm.left_behind.is_empty(), "{:?}", sm.left_behind);
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
    assert_eq!(sm.state, SubmoduleState::Missing, "folder deleted");
    assert_eq!(sm.checked_out, None);
    assert!(!sm.is_initialized());
    cleanup(&parent);
}

#[test]
fn added_submodule_is_marked() {
    let lib = scratch_repo("sm-added-lib");
    write(
        &lib, "lib.txt", "lib
",
    );
    git(&lib, &["add", "."]);
    git(&lib, &["commit", "-q", "-m", "lib"]);
    let parent = scratch_repo("sm-added");
    write(
        &parent, "a.txt", "one
",
    );
    git(&parent, &["add", "."]);
    git(&parent, &["commit", "-q", "-m", "first"]);
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "libs/lib"]);
    let sm = only(&parent);
    assert!(sm.added, "staged, not committed");
    assert_eq!(sm.state, SubmoduleState::InSync);
    git(&parent, &["commit", "-q", "-m", "add lib"]);
    assert!(!only(&parent).added);

    // Added by hand and never cloned: a gitlink, a .gitmodules entry and an empty folder.
    let mut text = std::fs::read_to_string(parent.join(".gitmodules")).unwrap();
    text.push_str(&format!(
        "[submodule \"libs/new\"]
	path = libs/new
	url = {url}
"
    ));
    write(&parent, ".gitmodules", &text);
    git(&parent, &["add", ".gitmodules"]);
    let cacheinfo = format!("160000,{},libs/new", head(&lib));
    git(&parent, &["update-index", "--add", "--cacheinfo", &cacheinfo]);
    std::fs::create_dir_all(parent.join("libs").join("new")).unwrap();
    let list = subs(&parent);
    let new = list.iter().find(|m| m.path == "libs/new").expect("the new gitlink");
    assert!(new.added && new.state == SubmoduleState::Uninitialized, "{new:?}");
    assert!(list.iter().any(|m| m.path == "libs/lib" && !m.added));
    let _ = std::fs::remove_dir_all(&lib);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn branch_and_clone_url() {
    let (parent, _sub) = with_submodule("sm-branch-url");
    let lib = lib_dir(&parent);
    let sm = only(&parent);
    assert_eq!(sm.branch, None);
    let config = || Repository::open(&parent).unwrap().config().unwrap().snapshot().unwrap().get_string("submodule.libs/lib.url").ok();
    let configured = config().expect("an initialized submodule has its URL in the config");
    assert_eq!(sm.clone_url.as_deref(), Some(configured.as_str()));

    git(&parent, &["config", "-f", ".gitmodules", "submodule.libs/lib.branch", "dev"]);
    assert_eq!(only(&parent).branch.as_deref(), Some("dev"));

    git(&parent, &["submodule", "deinit", "-q", "-f", "libs/lib"]);
    let _ = unset_url(&parent);
    assert_eq!(config(), None);
    git(&parent, &["remote", "add", "origin", "https://example.com/o/parent.git"]);
    let name = lib.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(only(&parent).clone_url, Some(format!("https://example.com/o/{name}")));
    cleanup(&parent);
}

/// `git config --unset submodule.libs/lib.url`, which fails harmlessly when deinit dropped it.
fn unset_url(dir: &Path) -> bool {
    crate::gitcli::run(dir, &crate::gitcli::args(&["config", "--unset", "submodule.libs/lib.url"])).0
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
    assert!(status_at(&parent, true).is_ok(), "status reads the files without the submodules");
    cleanup(&parent);
}
