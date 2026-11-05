//! A repository whose own files name network places (a submodule's `.git` file, a nested
//! repository's, a linked worktree's record; all `testutil::net_place()`, never touched) still
//! loads, with none of them followed.

use super::*;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// `p` as git writes it in a `.git` file or record: forward slashes.
fn named(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// A repository with a.txt committed and modified since, a gitlink `sub` (listed in
/// .gitmodules) whose `.git` names a network place, a nested `inner` whose `.git` does too, and
/// a linked worktree `wt` whose record names a folder there. The places are written after the
/// last git command, and git's own status is kept off (no speed-ups), so git never sees them.
pub(super) fn offsite_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, &[("core.fsmonitor", "false"), ("core.untrackedCache", "false")]);
    write(&dir, "a.txt", "a\n");
    write(&dir, ".gitmodules", "[submodule \"sub\"]\n\tpath = sub\n\turl = ../sub\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap().to_string();
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")]);
    git(&dir, &["commit", "-q", "-m", "sub"]);
    for folder in ["sub", "inner"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        write(&dir.join(folder), ".git", &format!("gitdir: {}\n", named(&net_place().join(folder).join(".git"))));
    }
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    write(&admin, "HEAD", &format!("{head}\n"));
    write(&admin, "commondir", "../..\n");
    write(&admin, "gitdir", &format!("{}\n", named(&net_place().join("wt/.git"))));
    write(&dir, "a.txt", "changed\n");
    dir
}

#[test]
fn the_repository_still_loads_when_its_contents_name_network_places() {
    let dir = offsite_repo("offsite-load");
    let snap = load(&dir, &LoadOptions { worktree_wips: true, ..LoadOptions::default() }).unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt" && f.status == 'M'), "{:?}", snap.unstaged);
    let sub = snap.submodules.iter().find(|s| s.path == "sub").expect("sub is listed");
    assert_eq!((sub.state, sub.checked_out), (SubmoduleState::Uninitialized, None));
    assert!(sub.left_behind.is_empty());
    let wt = snap.worktrees.iter().find(|w| w.name.as_deref() == Some("wt")).expect("wt is listed");
    assert!(!wt.missing && !wt.prunable, "{wt:?}");
    assert_eq!((wt.folder.is_none(), wt.read), (true, ChangesRead::NotCounted));
    assert!(wt.head.is_some(), "read from its admin dir");
    let _ = std::fs::remove_dir_all(&dir);
}
