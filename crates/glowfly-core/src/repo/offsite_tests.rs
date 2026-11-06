//! A repository whose own files name network places (a submodule's `.git` file, a nested
//! repository's, a linked worktree's record; all `testutil::net_place()`, never touched) still
//! loads, with none of them followed, and its diffs to the working tree leave submodules out.

use super::*;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// A repository with a.txt committed and modified since, a gitlink `sub` (listed in
/// .gitmodules) whose `.git` names a network place, a nested `inner` whose `.git` does too, and
/// a linked worktree `wt` whose record names a folder there. The places are written after the
/// last git command, and git's own status is kept off (no speed-ups), so git never sees them.
pub(super) fn offsite_repo(name: &str) -> PathBuf {
    repo_with(name, false, true)
}

/// `offsite_repo`, with a checked-out submodule `local` one commit ahead of its gitlink (a
/// changed submodule) when `moved`, and with `sub` and `inner` left empty and no `wt` unless
/// `offsite`.
fn repo_with(name: &str, moved: bool, offsite: bool) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, &[("core.fsmonitor", "false"), ("core.untrackedCache", "false")]);
    write(&dir, "a.txt", "a\n");
    let mut modules = String::from("[submodule \"sub\"]\n\tpath = sub\n\turl = ../sub\n");
    if moved {
        modules.push_str("[submodule \"local\"]\n\tpath = local\n\turl = ./local\n");
    }
    write(&dir, ".gitmodules", &modules);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap().to_string();
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")]);
    git(&dir, &["commit", "-q", "-m", "sub"]);
    if moved {
        let local = dir.join("local");
        std::fs::create_dir_all(&local).unwrap();
        init_repo(&local);
        git(&local, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let one = git2::Repository::open(&local).unwrap().head().unwrap().target().unwrap().to_string();
        git(&local, &["commit", "-q", "--allow-empty", "-m", "two"]);
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{one},local")]);
        git(&dir, &["commit", "-q", "-m", "local"]);
    }
    for folder in ["sub", "inner"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        if offsite {
            write(&dir.join(folder), ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join(folder).join(".git"))));
        }
    }
    if offsite {
        let admin = dir.join(".git/worktrees/wt");
        std::fs::create_dir_all(&admin).unwrap();
        write(&admin, "HEAD", &format!("{head}\n"));
        write(&admin, "commondir", "../..\n");
        write(&admin, "gitdir", &format!("{}\n", git_spelling(&net_place().join("wt/.git"))));
    }
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
    assert!(!wt.missing && !wt.prunable && !wt.current, "{wt:?}");
    assert_eq!((wt.folder.is_none(), wt.read), (true, ChangesRead::NotCounted));
    assert!(wt.head.is_some(), "read from its admin dir");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_worktree_record_naming_a_network_place_is_never_compared_to_the_current_folder() {
    let dir = offsite_repo("offsite-wt-current");
    // `paths::same_folder` would open the record's folder (`testutil::never_net_place` fails).
    let list = worktrees_at(&dir).unwrap();
    let wt = list.iter().find(|w| w.name.as_deref() == Some("wt")).expect("wt is listed");
    assert!(!wt.current && list[0].current, "{list:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diffs_to_the_working_tree_leave_submodules_out_when_one_names_a_network_place() {
    for offsite in [false, true] {
        let dir = repo_with(&format!("offsite-diff-{offsite}"), true, offsite);
        let repo = git2::Repository::open(&dir).unwrap();
        let head = repo.head().unwrap().target().unwrap();
        let lists_local = |files: &[FileEntry]| files.iter().any(|f| f.path == "local");
        // Comparing a commit to the working tree.
        assert_eq!(lists_local(&compare_workdir(&dir, head).unwrap().files), !offsite, "offsite {offsite}");
        // The unstaged diff the line counts read.
        let mut o = git2::DiffOptions::new();
        let diff = repo.diff_index_to_workdir(None, Some(crate::git_file::skip_offsite_submodules(&repo, &mut o))).unwrap();
        let in_diff = diff.deltas().any(|d| d.new_file().path() == Some(Path::new("local")));
        assert_eq!(in_diff, !offsite, "offsite {offsite}");
        drop(diff);
        let counts = wip_line_counts(&dir).unwrap();
        assert_eq!(counts.unstaged.total.added, 1, "a.txt is still counted: {counts:?}");
        // The changed files that keep a worktree from being removed: a.txt, and `local` unless
        // submodules are left out.
        assert_eq!(crate::worktree_ops::dirty_count(&dir).unwrap(), if offsite { 1 } else { 2 }, "offsite {offsite}");
        drop(repo);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
