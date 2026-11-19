//! The folder the user opens: one whose `.git` file, `commondir`, worktree record, config
//! include or alternates name `testutil::net_place()` (never touched) isn't opened and says
//! which place; an ordinary repository, a linked worktree, a submodule's checkout and a folder
//! inside one open as before, as does a work tree holding something named `commondir`. A file
//! whose value can't be told (two lines, a relative `commondir`) is refused as that, not as a
//! network place. These are plain text and need no link; the one that does (a link in the
//! opened path, which libgit2 resolves on Unix) keeps its other assertions above the link.

use super::*;
use crate::error::Error;
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, git_spelling, link_to, net_place, scratch_repo, with_submodule, write};

/// A repository with one commit.
fn repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    dir
}

/// The place the check of `dir` names, when it is another network place.
fn place_of(dir: &Path) -> Option<PathBuf> {
    check(dir, true).err().filter(|r| r.network).map(|r| r.place)
}

/// Whether opening `dir` is refused naming `place`, by the check, the open and the load alike.
fn refused(dir: &Path, place: &Path) -> bool {
    let shown = crate::native_path::shown(place);
    let names = |e: Error| matches!(e, Error::NetworkPointer(p) if p == shown);
    place_of(dir) == Some(place.to_path_buf())
        && crate::safe_directory::discover(dir).err().is_some_and(names)
        && crate::safe_directory::open(dir).err().is_some_and(names)
        && load(dir, &LoadOptions::default()).err().is_some_and(names)
}

#[test]
fn a_git_file_naming_a_network_place_is_not_opened() {
    let dir = scratch_repo("opened-git-file");
    let inner = dir.join("inner");
    std::fs::create_dir_all(inner.join("src")).unwrap();
    let place = net_place().join("x/.git");
    write(&inner, ".git", &format!("gitdir: {}\n", git_spelling(&place)));
    let named = PathBuf::from(git_spelling(&place));
    assert!(refused(&inner, &named));
    // From a folder inside it too, as libgit2 looks above.
    assert_eq!(place_of(&inner.join("src")), Some(named));
    assert!(matches!(load(&inner.join("src"), &LoadOptions::default()), Err(Error::NetworkPointer(_))));
    // The repository around it is not what was opened, and opens.
    assert!(load(&dir, &LoadOptions::default()).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commondir_naming_a_network_place_is_not_opened() {
    let dir = repo("opened-commondir");
    let place = net_place().join("repo/.git");
    write(&dir.join(".git"), "commondir", &format!("{}\n", git_spelling(&place)));
    assert!(refused(&dir, &PathBuf::from(git_spelling(&place))));
    std::fs::remove_file(dir.join(".git/commondir")).unwrap();
    // libgit2 reads one in the folder itself as well, before it looks above.
    let plain = dir.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    write(&plain, "commondir", &format!("{}\n", git_spelling(&place)));
    assert_eq!(place_of(&plain), Some(PathBuf::from(git_spelling(&place))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_config_include_naming_a_network_place_is_not_opened() {
    let dir = repo("opened-include");
    let place = net_place().join("shared.cfg");
    git(&dir, &["config", "include.path", &git_spelling(&place)]);
    assert!(refused(&dir, &PathBuf::from(git_spelling(&place))));
    git(&dir, &["config", "--unset", "include.path"]);
    assert!(load(&dir, &LoadOptions::default()).is_ok());
    // The worktree's own config file is read too.
    write(&dir.join(".git"), "config.worktree", &format!("[include]\n\tpath = {}\n", git_spelling(&place)));
    assert!(refused(&dir, &PathBuf::from(git_spelling(&place))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_alternates_line_naming_a_network_place_is_not_opened() {
    let dir = repo("opened-alternates");
    let place = net_place().join("store/objects");
    write(&dir.join(".git/objects/info"), "alternates", &format!("{}\n", git_spelling(&place)));
    assert!(refused(&dir, &PathBuf::from(git_spelling(&place))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_whose_record_names_a_network_place_is_not_opened() {
    let dir = repo("opened-record");
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    assert!(load(&linked, &LoadOptions::default()).is_ok(), "as git made it");
    let place = net_place().join("side/.git");
    write(&dir.join(".git/worktrees").join(linked.file_name().unwrap()), "gitdir", &format!("{}\n", git_spelling(&place)));
    assert!(refused(&linked, &PathBuf::from(git_spelling(&place))));
    assert!(load(&dir, &LoadOptions::default()).is_ok(), "the main worktree opens: the other is only listed");
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ordinary_repositories_worktrees_and_submodules_open_as_before() {
    let (parent, sub) = with_submodule("opened-usual");
    let linked = parent.with_file_name(format!("{}-side", parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&parent, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    assert!(sub.join(".git").is_file(), "a submodule's .git file points inside the superproject");
    for dir in [&parent, &sub, &linked, &parent.join("libs"), &parent.join(".git")] {
        assert_eq!(check(dir, true), Ok(()), "{}", dir.display());
    }
    for dir in [&parent, &sub, &linked, &parent.join("libs")] {
        let snap = load(dir, &LoadOptions::default());
        assert!(snap.is_ok(), "{}: {:?}", dir.display(), snap.err());
    }
    // Includes and alternates on this computer are followed as before.
    let store = scratch_repo("opened-usual-store");
    write(&parent, "local.cfg", "[user]\n\tname = Someone\n");
    git(&parent, &["config", "include.path", "../local.cfg"]);
    write(&parent.join(".git/objects/info"), "alternates", &format!("{}\n", git_spelling(&store.join(".git/objects"))));
    assert!(load(&parent, &LoadOptions::default()).is_ok());
    // A folder with no repository is called that, not refused.
    let plain = crate::paths::temp_dir().join(format!("glowfly-test-opened-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(check(&plain, false), Ok(()));
    assert!(matches!(crate::safe_directory::open(&plain), Err(Error::NotARepository(_))));
    for d in [&linked, &parent, &store, &plain] {
        let _ = std::fs::remove_dir_all(d);
    }
    let _ = std::fs::remove_dir_all(parent.with_file_name(format!("{}-lib", parent.file_name().unwrap().to_string_lossy())));
}

#[test]
fn a_work_tree_holding_something_named_commondir_opens() {
    let dir = repo("opened-commondir-name");
    // libgit2 reads a `commondir` only in a folder it tries as a git dir, after its `.git`.
    std::fs::create_dir_all(dir.join("commondir")).unwrap();
    assert_eq!(check(&dir, true), Ok(()));
    assert!(load(&dir, &LoadOptions::default()).is_ok(), "a folder of that name");
    std::fs::remove_dir(dir.join("commondir")).unwrap();
    write(&dir, "commondir", "notes about the shared folder\n");
    assert_eq!(check(&dir, true), Ok(()));
    assert!(load(&dir, &LoadOptions::default()).is_ok(), "a text file of that name");
    // From a folder inside too: a folder named so is passed over on the way up.
    std::fs::create_dir_all(dir.join("docs/commondir")).unwrap();
    assert!(load(&dir.join("docs"), &LoadOptions::default()).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn git_data_that_cant_be_told_is_refused_as_that() {
    let dir = scratch_repo("opened-untold");
    let inner = dir.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    // Two lines: libgit2 takes them as one path.
    write(&inner, ".git", "gitdir: ../.git\nmore\n");
    let file = inner.join(".git");
    assert_eq!(check(&inner, true), Err(Refused { place: file.clone(), network: false }));
    let shown = crate::native_path::shown(&file);
    assert!(matches!(load(&inner, &LoadOptions::default()), Err(Error::UncheckedGitData(w)) if w == shown));
    std::fs::remove_file(&file).unwrap();
    // A relative `commondir` that libgit2 would read from the process's own folder.
    let main = repo("opened-untold-main");
    write(&main.join(".git"), "commondir", "elsewhere\n");
    assert_eq!(check(&main, true), Err(Refused { place: main.join(".git/commondir"), network: false }));
    assert!(matches!(crate::safe_directory::discover(&main), Err(Error::UncheckedGitData(_))));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&main);
}

#[test]
fn a_link_in_the_opened_path_is_resolved_as_libgit2_does() {
    let top = crate::paths::temp_dir().join(format!("glowfly-test-opened-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&top);
    std::fs::create_dir_all(top.join("real/inner")).unwrap();
    let place = git_spelling(&net_place().join("x/.git"));
    write(&top.join("real"), ".git", &format!("gitdir: {place}\n"));
    assert_eq!(place_of(&top.join("real/inner")), Some(PathBuf::from(&place)), "by its own path");
    if link_to(&top.join("real/inner"), &top.join("link"), true) {
        // Unix: libgit2 looks above the folder the link leads to. Windows: above the link.
        let found = place_of(&top.join("link"));
        assert_eq!(found, cfg!(unix).then(|| PathBuf::from(&place)));
        assert!(load(&top.join("link"), &LoadOptions::default()).is_err());
    }
    let _ = std::fs::remove_dir_all(&top);
}
