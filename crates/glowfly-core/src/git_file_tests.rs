//! Following the git dirs a repository's files name, and refusing the ones on a network place:
//! `.git` files, `commondir`, a nested repository's config and links, worktree records, admin
//! dirs and gitlinks. Every network path is `testutil::net_place()`, which is never touched.

use super::*;
use crate::testutil::{git, git_spelling, link_to, net_place, scratch_repo, write};

#[test]
fn a_git_file_naming_a_network_place_is_refused() {
    let dir = scratch_repo("git-file-net");
    let inner = dir.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let mut places = vec![git_spelling(&net_place().join("x/.git"))];
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
    write(&admin, "commondir", &format!("{}\n", git_spelling(&place)));
    assert!(matches!(common_dir_of(&admin, &dir), Named::Refused(_)));
    write(&admin, "commondir", "../..\n");
    assert_eq!(common_dir_of(&admin, &dir), Named::At(crate::paths::resolve(&dir.join(".git"))));
    std::fs::remove_file(admin.join("commondir")).unwrap();
    assert_eq!(common_dir_of(&admin, &dir), Named::None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commondir_is_read_up_to_its_first_nul() {
    let dir = scratch_repo("git-file-commondir-nul");
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    // libgit2 reads the file as a C string: nothing past the NUL is used.
    write(&admin, "commondir", &format!("../..\0{}\n", git_spelling(&net_place())));
    assert_eq!(common_dir_of(&admin, &dir), Named::At(crate::paths::resolve(&dir.join(".git"))));
    if link_to(&net_place(), &admin.join("link"), true) {
        write(&admin, "commondir", "./link\0\n");
        assert!(matches!(common_dir_of(&admin, &dir), Named::Refused(_)), "the text before the NUL is checked");
    }
    // The whole file is trimmed before the NUL ends it: what is left before the NUL stays.
    let file = admin.join("commondir");
    assert_eq!(value_in(b"../.. \0z"), Value::Text("../.. ".into()));
    write(&admin, "commondir", "../..\n\0");
    assert_eq!(common_dir_of(&admin, &dir), Named::Refused(file), "a LF before the NUL is part of the path");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commondir_is_trimmed_and_resolved_as_libgit2_does() {
    let dir = scratch_repo("git-file-commondir-trim");
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    let common = Named::At(crate::paths::resolve(&dir.join(".git")));
    // `git_str_rtrim`: TAB, space, VT, FF, CR and LF.
    write(&admin, "commondir", "../..\t \x0b\x0c\r\n");
    assert_eq!(common_dir_of(&admin, &dir), common);
    // A second line is part of the path to libgit2.
    write(&admin, "commondir", &format!("../..\n{}\n", git_spelling(&net_place())));
    assert_eq!(common_dir_of(&admin, &dir), Named::Refused(admin.join("commondir")));
    // libgit2 reads these from the process's own folder.
    for named in ["sub\t\n", "", " ../..", r"..\.."] {
        write(&admin, "commondir", named);
        assert_eq!(common_dir_of(&admin, &dir), Named::Refused(admin.join("commondir")), "{named:?}");
    }
    if link_to(&net_place(), &admin.join("link"), true) {
        write(&admin, "commondir", "./link\t \n");
        assert!(matches!(common_dir_of(&admin, &dir), Named::Refused(_)), "the name without its trailing TAB is checked");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_git_file_is_read_whole_as_libgit2_reads_it() {
    let dir = scratch_repo("git-file-whole");
    let inner = dir.join("inner");
    std::fs::create_dir_all(inner.join("sub")).unwrap();
    let dot = inner.join(".git");
    // libgit2 takes every line as one path: `d<LF>/../..` climbs to the network place.
    write(&inner, ".git", &format!("gitdir: d\n/../../../../../../../..{}\n", git_spelling(&net_place().join("x"))));
    assert_eq!(dirs_of(&inner, &dir), Found::Refused(dot.clone()));
    assert_eq!(other_repo(&inner, &dir), Found::Refused(dot.clone()));
    // Only ASCII whitespace is trimmed, after `gitdir:` and at the end.
    write(&inner, ".git", "gitdir:\t \x0b./sub \x0c\r\n");
    assert_eq!(dirs_of(&inner, &dir), Found::Repo(RepoDirs { git_dir: inner.join("./sub"), common: inner.join("./sub") }));
    // libgit2 trims before the NUL takes effect: `x<SP>` and `x<LF>` are its paths.
    assert_eq!(value_in(b"gitdir: x \0y"), Value::Text("gitdir: x ".into()));
    write(&inner, ".git", "gitdir: x\n\0");
    assert_eq!(dirs_of(&inner, &dir), Found::Refused(dot.clone()), "a LF before the NUL");
    // A locale may count U+00A0 as a space: a value starting or ending with one isn't followed.
    for text in ["gitdir: link\u{a0}\n", "gitdir:\u{a0}link\n", "gitdir: ./sub\u{3000}"] {
        write(&inner, ".git", text);
        assert_eq!(dirs_of(&inner, &dir), Found::Refused(dot.clone()), "{text:?}");
    }
    assert_eq!(value_in("gitdir: a\u{a0}b".as_bytes()), Value::Text("gitdir: a\u{a0}b".into()), "inside a name it is kept");
    // Too big to read whole: libgit2 would still read it.
    write(&inner, ".git", &format!("gitdir: {}{}", git_spelling(&net_place()), " ".repeat(SMALL_FILE as usize)));
    assert_eq!(dirs_of(&inner, &dir), Found::Refused(dot));
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
    let place = git_spelling(&net_place().join("x.cfg"));
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
    crate::testutil::set_config(&inner, &[("include.path", &git_spelling(&dir.join("local.cfg")))]);
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
    write(&admin, "gitdir", &format!("{}\n", git_spelling(&wt.join(".git"))));
    assert_eq!(worktree_folder(&admin, &common), Named::At(PathBuf::from(git_spelling(&wt.join(".git")))));
    write(&admin, "gitdir", "../../../wt/.git\n");
    assert_eq!(worktree_folder(&admin, &common), Named::At(admin.join("../../../wt/.git")), "joined to the admin dir, not the cwd");
    write(&admin, "gitdir", &format!("{}\n", git_spelling(&net_place().join("wt/.git"))));
    assert!(matches!(worktree_folder(&admin, &common), Named::Refused(_)));
    write(
        &admin,
        "gitdir",
        "../../../wt/.git	 
",
    );
    assert_eq!(worktree_folder(&admin, &common), Named::At(admin.join("../../../wt/.git")), "ASCII whitespace trimmed");
    // libgit2 takes every line as one path, joined lexically: no folder `x<LF>` is needed.
    let record = admin.join("gitdir");
    write(
        &admin,
        "gitdir",
        &format!(
            "x
/../../../..{}
",
            git_spelling(&net_place().join("wt/.git"))
        ),
    );
    assert_eq!(worktree_folder(&admin, &common), Named::Refused(record.clone()));
    // A relative record not starting with `./` or `../` is read from the process's own folder.
    write(
        &admin, "gitdir", "wt/.git
",
    );
    assert_eq!(worktree_folder(&admin, &common), Named::Refused(record));
    write(&admin, "gitdir", " \n");
    assert_eq!(worktree_folder(&admin, &common), Named::Refused(admin.join("gitdir")), "an empty record");
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
    write(&dir.join("far"), ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("far/.git"))));
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

/// Writes an admin dir `<common>/worktrees/<name>` holding `files` (name, text).
fn admin_dir(common: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = common.join("worktrees").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for (f, text) in files {
        write(&dir, f, text);
    }
    dir
}

#[test]
fn worktree_admins_list_as_libgit2_does_without_following_links() {
    let dir = scratch_repo("git-file-admins");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "--detach", &wt.display().to_string()]);
    let common = dir.join(".git");
    let listed = |name: &str| worktree_admins(&common).into_iter().find(|a| a.name == name);
    let real = listed(&wt.file_name().unwrap().to_string_lossy()).expect("the real worktree");
    assert!(real.near && crate::paths::same_folder(&real.folder, &wt), "{real:?}");
    let gitdir = format!("{}\n", git_spelling(&dir.join("elsewhere/.git")));
    admin_dir(&common, "no-head", &[("commondir", "../..\n"), ("gitdir", &gitdir)]);
    assert_eq!(listed("no-head"), None, "not an admin dir without HEAD");
    let far = format!("{}\n", git_spelling(&net_place()));
    admin_dir(&common, "far-common", &[("commondir", &far), ("gitdir", &gitdir), ("HEAD", "ref: refs/heads/main\n")]);
    assert_eq!(listed("far-common").map(|a| a.near), Some(false), "a commondir naming a network place");
    let head_link = admin_dir(&common, "head-link", &[("commondir", "../..\n"), ("gitdir", &gitdir)]);
    if link_to(&net_place().join("HEAD"), &head_link.join("HEAD"), false) {
        assert_eq!(listed("head-link").map(|a| a.near), Some(false), "a HEAD linked to a network place");
        let gitdir_link = admin_dir(&common, "gitdir-link", &[("commondir", "../..\n"), ("HEAD", "ref: refs/heads/main\n")]);
        assert!(link_to(&net_place().join("gitdir"), &gitdir_link.join("gitdir"), false));
        assert_eq!(listed("gitdir-link"), None, "no gitdir record to read");
        assert!(link_to(&net_place().join("wt"), &common.join("worktrees/dir-link"), true));
        assert_eq!(listed("dir-link"), None, "an admin dir that is a link to a network place");
        let root = common.join("worktrees");
        std::fs::rename(&root, common.join("worktrees-kept")).unwrap();
        assert!(link_to(&net_place().join("worktrees"), &root, true));
        assert_eq!(worktree_admins(&common), [], "a worktrees folder that is a link to a network place");
        let _ = std::fs::remove_dir(&root).or_else(|_| std::fs::remove_file(&root));
        std::fs::rename(common.join("worktrees-kept"), &root).unwrap();
        let link = root.join("dir-link");
        let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    }
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn admin_locked_reads_the_reason_and_never_follows_a_far_link() {
    let dir = scratch_repo("git-file-admin-locked");
    let admin = admin_dir(&dir.join(".git"), "wt", &[]);
    assert_eq!(admin_locked(&admin), None, "not locked");
    write(&admin, "locked", "because\n");
    assert_eq!(admin_locked(&admin).as_deref(), Some("because"));
    std::fs::remove_file(admin.join("locked")).unwrap();
    if link_to(&net_place().join("locked"), &admin.join("locked"), false) {
        assert_eq!(admin_locked(&admin).as_deref(), Some(""), "locked, its reason never read");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
