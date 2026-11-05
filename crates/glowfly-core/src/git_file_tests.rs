//! Following the git dirs a repository's files name, and refusing the ones on a network place:
//! `.git` files, `commondir`, a nested repository's config and links, worktree records and
//! gitlinks. Every network path is `testutil::net_place()`, which is never touched.

use super::*;
use crate::testutil::{git, link_to, net_place, scratch_repo, write};

/// `p` as a `.git` file or record names it: forward slashes, as git writes them.
fn named(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

#[test]
fn a_git_file_naming_a_network_place_is_refused() {
    let dir = scratch_repo("git-file-net");
    let inner = dir.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let mut places = vec![named(&net_place().join("x/.git"))];
    if cfg!(windows) {
        places.extend(
            [r"//glowfly-test.invalid/share/x", r"\\?\UNC\glowfly-test.invalid\share\x", r"\\.\UNC\glowfly-test.invalid\share\x"]
                .map(str::to_owned),
        );
    }
    for place in places {
        write(&inner, ".git", &format!("gitdir: {place}\n"));
        let refused = Found::Refused(PathBuf::from(&place));
        assert_eq!(dirs_of(&inner, &dir), refused, "{place}");
        assert_eq!(other_repo(&inner, &dir), refused, "{place}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commondir_naming_a_network_place_is_refused() {
    let dir = scratch_repo("git-file-commondir");
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    let place = net_place().join("repo/.git");
    write(&admin, "commondir", &format!("{}\n", named(&place)));
    assert!(matches!(common_dir_of(&admin, &dir), Named::Refused(_)));
    write(&admin, "commondir", "../..\n");
    assert_eq!(common_dir_of(&admin, &dir), Named::At(crate::paths::resolve(&dir.join(".git"))));
    std::fs::remove_file(admin.join("commondir")).unwrap();
    assert_eq!(common_dir_of(&admin, &dir), Named::None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodule_style_git_file_is_followed() {
    let dir = scratch_repo("git-file-absorbed");
    let modules = dir.join(".git/modules/sub");
    git2::Repository::init_bare(&modules).unwrap();
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    write(&sub, ".git", "gitdir: ../.git/modules/sub\n");
    let git_dir = sub.join("../.git/modules/sub");
    assert_eq!(dirs_of(&sub, &dir), Found::Repo(RepoDirs { git_dir: git_dir.clone(), common: git_dir.clone() }));
    assert_eq!(other_repo(&sub, &dir), Found::Repo(RepoDirs { git_dir: git_dir.clone(), common: git_dir }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_nested_config_naming_a_network_place_is_refused() {
    let dir = scratch_repo("git-file-config");
    let inner = dir.join("inner");
    git2::Repository::init(&inner).unwrap();
    let place = named(&net_place().join("x.cfg"));
    for (key, value) in
        [("include.path", place.as_str()), ("core.worktree", &place), ("core.excludesFile", &place), ("core.attributesFile", &place)]
    {
        let config = inner.join(".git/config");
        let before = std::fs::read_to_string(&config).unwrap();
        crate::testutil::set_config(&inner, &[(key, value)]);
        assert!(matches!(dirs_of(&inner, &dir), Found::Repo(_)), "{key}: cheap and still a repository");
        assert!(matches!(other_repo(&inner, &dir), Found::Refused(_)), "{key}");
        std::fs::write(&config, before).unwrap();
    }
    write(&dir, "local.cfg", "[core]\n\tautocrlf = false\n");
    crate::testutil::set_config(&inner, &[("include.path", &named(&dir.join("local.cfg")))]);
    assert!(matches!(other_repo(&inner, &dir), Found::Repo(_)), "a local include");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_link_in_a_nested_git_dir_to_a_network_place_is_refused() {
    let dir = scratch_repo("git-file-links");
    let inner = dir.join("inner");
    git2::Repository::init(&inner).unwrap();
    let git_dir = inner.join(".git");
    std::fs::create_dir_all(git_dir.join("info")).unwrap();
    for (name, target) in [("config", "c"), ("info/exclude", "e")] {
        let file = git_dir.join(name);
        let kept = file.with_extension("kept");
        let _ = std::fs::rename(&file, &kept);
        if !link_to(&net_place().join(target), &file, false) {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        assert_eq!(other_repo(&inner, &dir), Found::Refused(file.clone()), "{name}");
        std::fs::remove_file(&file).unwrap();
        let _ = std::fs::rename(&kept, &file);
    }
    assert!(matches!(other_repo(&inner, &dir), Found::Repo(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn worktree_records_absolute_relative_and_network() {
    let dir = scratch_repo("git-file-records");
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    let common = dir.join(".git");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    write(&admin, "gitdir", &format!("{}\n", named(&wt.join(".git"))));
    assert_eq!(worktree_folder(&admin, &common), Named::At(PathBuf::from(named(&wt.join(".git")))));
    write(&admin, "gitdir", "../../../wt/.git\n");
    assert_eq!(worktree_folder(&admin, &common), Named::At(admin.join("../../../wt/.git")), "joined to the admin dir, not the cwd");
    write(&admin, "gitdir", &format!("{}\n", named(&net_place().join("wt/.git"))));
    assert!(matches!(worktree_folder(&admin, &common), Named::Refused(_)));
    std::fs::remove_file(admin.join("gitdir")).unwrap();
    assert_eq!(worktree_folder(&admin, &common), Named::None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offsite_gitlinks_lists_only_refused_ones() {
    let dir = scratch_repo("git-file-gitlinks");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap().to_string();
    for name in ["far", "near", "linked"] {
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{head},{name}")]);
    }
    for name in ["far", "near"] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
    }
    write(&dir.join("far"), ".git", &format!("gitdir: {}\n", named(&net_place().join("far/.git"))));
    git2::Repository::init(dir.join("near")).unwrap();
    let repo = git2::Repository::open(&dir).unwrap();
    assert_eq!(offsite_gitlinks(&repo), [Offsite { path: "far".to_owned(), link: false }]);
    if link_to(&net_place().join("linked"), &dir.join("linked"), true) {
        let all = offsite_gitlinks(&repo);
        assert_eq!(all, [Offsite { path: "far".to_owned(), link: false }, Offsite { path: "linked".to_owned(), link: true }]);
    }
    drop(repo);
    let _ = std::fs::remove_dir_all(&dir);
}
