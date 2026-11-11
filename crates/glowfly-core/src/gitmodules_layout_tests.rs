//! Which submodule layouts count as plain: the ordinary ones git makes all do, and one whose
//! repository libgit2 would take differently from the folder looked at does not.

use super::tests::{one_commit, with_nested};
use crate::git_file::submodules_unread;
use crate::repo::submodules_at;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn sibling(parent: &Path, what: &str) -> PathBuf {
    let dir = parent.with_file_name(format!("{}-{what}", parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn lib_of(name: &str) -> PathBuf {
    crate::paths::temp_dir().join(format!("glowfly-test-{name}-lib-{}", std::process::id()))
}

fn plain_with_one(dir: &Path, what: &str) {
    let repo = Repository::open(dir).unwrap();
    assert!(!submodules_unread(&repo), "{what}");
    assert_eq!(submodules_at(dir).unwrap().len(), 1, "{what}");
}

#[test]
fn the_layouts_git_makes_are_plain() {
    // `git submodule add`: the git dir under the parent's, naming the folder back relatively.
    let (parent, _sub) = with_submodule("gitmodules-layouts");
    let config = std::fs::read_to_string(parent.join(".git/modules/libs/lib/config")).unwrap();
    assert!(config.contains("worktree = ../../../../libs/lib"), "{config}");
    plain_with_one(&parent, "submodule add");

    // `git clone --recurse-submodules` of it.
    let clone = sibling(&parent, "clone");
    let (from, to) = (git_spelling(&parent), git_spelling(&clone));
    git(&parent, &["-c", "protocol.file.allow=always", "clone", "-q", "--recurse-submodules", &from, &to]);
    assert!(clone.join("libs/lib/lib.txt").exists());
    plain_with_one(&clone, "clone --recurse-submodules");

    // A linked worktree of the parent with the submodule checked out in it.
    let tree = sibling(&parent, "tree");
    git(&parent, &["worktree", "add", "-q", "--detach", &git_spelling(&tree)]);
    git(&tree, &["-c", "protocol.file.allow=always", "submodule", "update", "-q", "--init"]);
    assert!(tree.join("libs/lib/lib.txt").exists());
    plain_with_one(&tree, "a linked worktree");

    for dir in [&clone, &tree, &parent, &lib_of("gitmodules-layouts")] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_nested_submodule_and_an_embedded_git_folder_are_plain() {
    let (parent, _sub) = with_nested("gitmodules-layouts-nested");
    plain_with_one(&parent, "a nested submodule");
    let _ = std::fs::remove_dir_all(lib_of("gitmodules-layouts-nested"));
    let _ = std::fs::remove_dir_all(&parent);

    let dir = embedded("gitmodules-layouts-embedded");
    plain_with_one(&dir, "an embedded .git folder");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repository with a gitlink `sub` whose folder holds its own `.git` folder.
fn embedded(name: &str) -> PathBuf {
    let dir = one_commit(name);
    let inner = dir.join("sub");
    std::fs::create_dir_all(&inner).unwrap();
    init_repo(&inner);
    write(&inner, "n.txt", "one\n");
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner"]);
    let head = Repository::open(&inner).unwrap().head().unwrap().target().unwrap().to_string();
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")]);
    dir
}

#[test]
fn a_conditional_include_in_a_submodules_config_counts_as_unread() {
    let (parent, _sub) = with_submodule("gitmodules-conditional");
    let repo = Repository::open(&parent).unwrap();
    let git_dir = parent.join(".git/modules/libs/lib");
    let config = std::fs::read_to_string(git_dir.join("config")).unwrap();

    // An include without a condition is read the same way here and by libgit2.
    std::fs::write(git_dir.join("extra.cfg"), "[core]\n\tautocrlf = false\n").unwrap();
    std::fs::write(git_dir.join("config"), format!("{config}[include]\n\tpath = extra.cfg\n")).unwrap();
    assert!(!submodules_unread(&repo));

    for section in ["[includeIf \"gitdir:**\"]", "[INCLUDEIF \"gitdir/i:**\"]", "[includeif \"onbranch:**\"]"] {
        std::fs::write(git_dir.join("config"), format!("{config}{section}\n\tpath = extra.cfg\n")).unwrap();
        assert!(submodules_unread(&repo), "{section}");
        assert_eq!(submodules_at(&parent).unwrap().iter().filter(|m| m.is_initialized()).count(), 0, "{section}");
    }
    // In a file the config includes, too.
    std::fs::write(git_dir.join("extra.cfg"), "[includeIf \"gitdir:**\"]\n\tpath = more.cfg\n").unwrap();
    std::fs::write(git_dir.join("config"), format!("{config}[include]\n\tpath = extra.cfg\n")).unwrap();
    assert!(submodules_unread(&repo));

    std::fs::write(git_dir.join("config"), &config).unwrap();
    assert!(!submodules_unread(&repo));
    let _ = std::fs::remove_dir_all(lib_of("gitmodules-conditional"));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn an_entry_named_like_a_git_dir_inside_a_git_folder_counts_as_unread() {
    for (n, name) in [".git", ".GIT", "git~1"].into_iter().enumerate() {
        let dir = embedded(&format!("gitmodules-inner-git-{n}"));
        let repo = Repository::open(&dir).unwrap();
        assert!(!submodules_unread(&repo));
        // Made directly: nothing git writes has this name.
        std::fs::create_dir(dir.join("sub").join(".git").join(name)).unwrap();
        assert!(submodules_unread(&repo), "{name}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
