//! `.gitmodules` include sections in every spelling, checked against what libgit2 and git
//! themselves take as one, and which files count as plain.

use super::*;
use crate::gitcli::{args, output};
use crate::testutil::*;

const SUB: &str = "[submodule \"a\"]\n\tpath = a\n\turl = written\n";

/// Whole `.gitmodules` texts, and whether each counts as naming an include section.
fn cases() -> Vec<(Vec<u8>, bool)> {
    let text = |s: &str, named: bool| (s.as_bytes().to_vec(), named);
    let after = |s: &str, named: bool| (format!("{SUB}{s}").into_bytes(), named);
    vec![
        after("[include]\n\tpath = inc.cfg\n", true),
        after("[INCLUDE]\n\tPATH = inc.cfg\n", true),
        after("[InClUdE] path = inc.cfg\n", true),
        after(" \t [include]path=inc.cfg\n", true),
        after("[include]\r\n\tpath = \"inc.cfg\"\r\n", true),
        after("\x0b[include]\n\tpath = inc.cfg\n", true),
        after("\x0c[include]\n\tpath = inc.cfg\n", true),
        after("[include]\n\tpath = in\\\nc.cfg\n", true),
        after("[include] ; note\n\tpath = inc.cfg # note\n", true),
        after("[includeIf \"gitdir:**\"]\n\tpath = inc.cfg\n", true),
        after("[includeIf \"gitdir/i:**\"]\n\tpath = inc.cfg\n", true),
        after("[INCLUDEIF \"onbranch:**\"]\n\tpath = inc.cfg\n", true),
        after("[includeif.gitdir:**]\n\tpath = inc.cfg\n", true),
        after("[includeIf \"hasconfig:remote.*.url:**\"]\n\tpath = inc.cfg\n", true),
        after("[include \"x\"]\n\tpath = inc.cfg\n", true),
        after("[include.x]\n\tpath = inc.cfg\n", true),
        text("\u{feff}[include]\n\tpath = inc.cfg\n", true),
        text("[include]\n\tpath = inc.cfg\n[submodule \"a\"]\n\turl = written\n", true),
        text("[submodule \"a\"] url = written\n[include] path = inc.cfg\n", true),
        text("[submodule \"a\"]\n\turl = written\\\n[include]\n\tpath = inc.cfg\n", true),
        text("[submodule \"a\"]\n\turl = \"written\n[include]\n\tpath = inc.cfg\n", true),
        (b"[submodule \"a\"]\n\turl = written\n[\xddnclude]\n\tpath = inc.cfg\n".to_vec(), true),
        (b"[submodule \"a\"]\n\turl = written\n\0[include]\n\tpath = inc.cfg\n".to_vec(), true),
        after("", false),
        after("[ include]\n\tpath = inc.cfg\n", false),
        after("[\"include\"]\n\tpath = inc.cfg\n", false),
        after("[submodule \"include\"]\n\tpath = inc.cfg\n", false),
        after("[submodule \"b\"]\n\tpath = include/lib\n\turl = https://example.invalid/include.git\n", false),
        after("# include\n; includeIf\n[core]\n\tinclude = inc.cfg\n", false),
        after("include.path = inc.cfg\n", false),
        after("[in\\\nclude]\n\tpath = inc.cfg\n", false),
        text("", false),
    ]
}

/// A folder holding `inc.cfg`, a config file with a URL for submodule `a`.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("inc.cfg"), "[submodule \"a\"]\n\turl = included\n").unwrap();
    dir
}

#[test]
fn every_spelling_libgit2_or_git_follows_counts_as_an_include_section() {
    let dir = folder("gitmodules-spellings");
    let mut followed = 0;
    for (text, named) in cases() {
        let shown = String::from_utf8_lossy(&text).into_owned();
        assert_eq!(names_include_section(&text), named, "{shown:?}");
        std::fs::write(dir.join(".gitmodules"), &text).unwrap();
        // libgit2 follows what it takes as an include section (the path spelled as libgit2
        // spells a work tree's, so the include resolves next to the file).
        let libgit2_follows = git2::Config::open(std::path::Path::new(&git_spelling(&dir.join(".gitmodules")))).is_ok_and(|c| {
            let mut included = false;
            if let Ok(entries) = c.entries(Some(r"^submodule\.a\.url$")) {
                let _ = entries.for_each(|e| included |= crate::config_entry::utf8(e) == Some("included"));
            }
            included
        });
        // git follows one only when asked to, which shows what its syntax takes as one.
        let asked = output(&dir, &args(&["config", "--includes", "-f", ".gitmodules", "--get-all", "submodule.a.url"]), "", &[], None);
        let git_follows = asked.stdout.lines().any(|l| l == "included");
        assert!(named || !(libgit2_follows || git_follows), "counts as an include section: {shown:?}");
        followed += usize::from(libgit2_follows);
        // Read the way git reads `.gitmodules`, the included value is never seen.
        let read = open(&dir).and_then(|c| c.get_string("submodule.a.url").ok());
        assert_ne!(read.as_deref(), Some("included"), "{shown:?}");
        assert_eq!(kind(&dir) == Gitmodules::Plain, !named, "{shown:?}");
        let plain = output(&dir, &args(&["config", "-f", ".gitmodules", "--get-all", "submodule.a.url"]), "", &[], None);
        assert!(!plain.stdout.lines().any(|l| l == "included"), "git itself never follows it: {shown:?}");
    }
    assert!(followed >= 8, "libgit2 follows the plain spellings, so the cases above test something: {followed}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_a_regular_file_of_bounded_size_is_plain() {
    let dir = folder("gitmodules-kind");
    assert_eq!(kind(&dir), Gitmodules::Absent);
    assert!(open(&dir).is_none());
    std::fs::write(dir.join(".gitmodules"), SUB).unwrap();
    assert_eq!(kind(&dir), Gitmodules::Plain);
    assert_eq!(open(&dir).unwrap().get_string("submodule.a.url").unwrap(), "written");

    let mut large = SUB.as_bytes().to_vec();
    large.resize(MAX_BYTES as usize + 1, b'\n');
    std::fs::write(dir.join(".gitmodules"), &large).unwrap();
    assert_eq!(kind(&dir), Gitmodules::Unread);
    assert!(open(&dir).is_none());

    std::fs::remove_file(dir.join(".gitmodules")).unwrap();
    std::fs::create_dir(dir.join(".gitmodules")).unwrap();
    assert_eq!(kind(&dir), Gitmodules::Unread);
    std::fs::remove_dir(dir.join(".gitmodules")).unwrap();

    // A link to a plain file is not read through.
    std::fs::write(dir.join("real"), SUB).unwrap();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink("real", dir.join(".gitmodules")).is_ok();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file("real", dir.join(".gitmodules")).is_ok();
    if linked {
        assert_eq!(kind(&dir), Gitmodules::Unread);
        assert!(open(&dir).is_none());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_whose_gitmodules_names_an_include_keeps_libgit2_from_its_submodules() {
    let (parent, _sub) = with_submodule("gitmodules-unread");
    let repo = git2::Repository::open(&parent).unwrap();
    assert!(!crate::git_file::submodules_unread(&repo));
    assert_eq!(crate::repo::submodules_at(&parent).unwrap().len(), 1);

    std::fs::write(parent.join("inc.cfg"), "[submodule \"libs/lib\"]\n\turl = included\n").unwrap();
    let written = std::fs::read_to_string(parent.join(".gitmodules")).unwrap();
    std::fs::write(parent.join(".gitmodules"), format!("{written}[include]\n\tpath = inc.cfg\n")).unwrap();
    let followed = repo.find_submodule("libs/lib").unwrap().url().map(str::to_owned);
    assert_eq!(followed.as_deref(), Some("included"), "libgit2 reads the file as an ordinary config file");

    assert!(crate::git_file::submodules_unread(&repo));
    assert_eq!(crate::repo::submodules_at(&parent).unwrap(), Vec::new());
    // The load and the status still work, with submodules left out.
    let snap = crate::repo::load(&parent, &crate::repo::LoadOptions::all(100)).unwrap();
    assert_eq!(snap.submodules, Vec::new());
    assert!(snap.unstaged.iter().any(|f| f.path == ".gitmodules"), "{:?}", snap.unstaged);
    let _ = std::fs::remove_dir_all(crate::paths::temp_dir().join(format!("glowfly-test-gitmodules-unread-lib-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&parent);
}

/// `with_submodule`, with a repository of its own checked out at `n` inside the submodule and
/// recorded there as a gitlink (and in the parent), then given one more commit.
fn with_nested(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let (parent, sub) = with_submodule(name);
    let inner = sub.join("n");
    std::fs::create_dir_all(&inner).unwrap();
    init_repo(&inner);
    write(&inner, "n.txt", "one\n");
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner"]);
    write(&sub, ".gitmodules", "[submodule \"n\"]\n\tpath = n\n\turl = ./n\n");
    write(&sub, "inc.cfg", "[submodule \"n\"]\n\tignore = all\n");
    git(&sub, &["add", ".gitmodules", "inc.cfg", "n"]);
    git(&sub, &["commit", "-q", "-m", "nested"]);
    git(&parent, &["add", "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "nested"]);
    write(&inner, "n.txt", "two\n");
    git(&inner, &["commit", "-q", "-a", "-m", "inner two"]);
    (parent, sub)
}

fn unstaged_paths(parent: &std::path::Path) -> Vec<String> {
    crate::repo::load(parent, &crate::repo::LoadOptions::all(100)).unwrap().unstaged.into_iter().map(|f| f.path).collect()
}

#[test]
fn a_checked_out_submodules_own_gitmodules_counts_too() {
    let (parent, sub) = with_nested("gitmodules-nested");
    let repo = git2::Repository::open(&parent).unwrap();
    // Plain at both levels: the submodule shows its inner repository's other commit.
    assert!(!crate::git_file::submodules_unread(&repo));
    assert_eq!(unstaged_paths(&parent), ["libs/lib"]);
    assert_eq!(crate::repo::submodules_at(&parent).unwrap().len(), 1);

    // The submodule's own file with an include section (committed there, so only the inner
    // repository's commit differs): submodules are left out of the parent's reads.
    write(&sub, ".gitmodules", "[submodule \"n\"]\n\tpath = n\n\turl = ./n\n[include]\n\tpath = inc.cfg\n");
    git(&sub, &["commit", "-q", "-m", "include", ".gitmodules"]);
    git(&parent, &["add", "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "include"]);
    assert_eq!(kind(&parent), Gitmodules::Plain);
    assert_eq!(kind(&sub), Gitmodules::Unread);
    assert!(crate::git_file::submodules_unread(&repo));
    assert_eq!(unstaged_paths(&parent), Vec::<String>::new());
    assert_eq!(crate::repo::submodules_at(&parent).unwrap(), Vec::new());
    assert!(crate::repo::load(&parent, &crate::repo::LoadOptions::all(100)).unwrap().submodules_unread);

    // Plain again: read as before.
    write(&sub, ".gitmodules", "[submodule \"n\"]\n\tpath = n\n\turl = ./n\n");
    git(&sub, &["commit", "-q", "-m", "plain", ".gitmodules"]);
    git(&parent, &["add", "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "plain"]);
    assert!(!crate::git_file::submodules_unread(&repo));
    assert_eq!(unstaged_paths(&parent), ["libs/lib"]);
    let _ = std::fs::remove_dir_all(crate::paths::temp_dir().join(format!("glowfly-test-gitmodules-nested-lib-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn a_submodule_whose_repository_names_another_work_tree_counts_as_unread() {
    let (parent, sub) = with_submodule("gitmodules-worktree");
    let repo = git2::Repository::open(&parent).unwrap();
    // `git submodule add` keeps the git dir under the parent's and names the folder back.
    let config = std::fs::read_to_string(parent.join(".git/modules/libs/lib/config")).unwrap();
    assert!(config.contains("worktree = "), "{config}");
    assert!(!crate::git_file::submodules_unread(&repo));
    assert_eq!(crate::repo::submodules_at(&parent).unwrap().len(), 1);

    let other = parent.with_file_name(format!("glowfly-test-gitmodules-worktree-other-{}", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    git(&sub, &["config", "core.worktree", &git_spelling(&other)]);
    assert!(crate::git_file::submodules_unread(&repo));
    assert_eq!(crate::repo::submodules_at(&parent).unwrap(), Vec::new());
    let _ = std::fs::remove_dir_all(&other);
    let _ = std::fs::remove_dir_all(crate::paths::temp_dir().join(format!("glowfly-test-gitmodules-worktree-lib-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&parent);
}

/// Adds a gitlink named `name` to `repo`'s index as it is, with no folder for it.
fn add_gitlink(repo: &git2::Repository, name: &[u8]) {
    let zero = git2::IndexTime::new(0, 0);
    let entry = git2::IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode: 0o160000,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: repo.head().unwrap().target().unwrap(),
        flags: 0,
        flags_extended: 0,
        path: name.to_vec(),
    };
    let mut index = repo.index().unwrap();
    index.add(&entry).unwrap();
    index.write().unwrap();
}

fn one_commit(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    dir
}

#[test]
fn a_gitlink_whose_name_cant_be_looked_at_counts_as_unread() {
    for (n, name) in [b"sub\xff".as_slice(), b"libs/sub\xfe\xff"].into_iter().enumerate() {
        let dir = one_commit(&format!("gitmodules-name-{n}"));
        let repo = git2::Repository::open(&dir).unwrap();
        assert!(!crate::git_file::submodules_unread(&repo));
        add_gitlink(&repo, name);
        assert!(unread_anywhere(&repo), "{name:?}");
        assert!(crate::git_file::offsite_gitlinks(&repo).iter().any(|o| o.link), "{name:?}");
        assert!(crate::git_file::submodules_unread(&repo), "{name:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
    // Names that read as `.git` on some file system; git's own name checks are off to add them.
    for (n, name) in ["git~1", "GIT~1", "libs/git~1", ".git.", ".git ", ".git:x", ".g\u{200c}it"].into_iter().enumerate() {
        let dir = one_commit(&format!("gitmodules-dotgit-{n}"));
        let repo = git2::Repository::open(&dir).unwrap();
        let head = repo.head().unwrap().target().unwrap().to_string();
        let info = format!("160000,{head},{name}");
        git(&dir, &["-c", "core.protectNTFS=false", "-c", "core.protectHFS=false", "update-index", "--add", "--cacheinfo", &info]);
        // Whether libgit2 reads such an index at all depends on the system; either way its
        // submodules count as unread.
        assert!(unread_anywhere(&repo), "{name:?}");
        assert!(crate::git_file::submodules_unread(&repo), "{name:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Where such a name is an ordinary folder name, a repository checked out in it is one libgit2's
/// status opens.
#[cfg(unix)]
#[test]
fn a_checked_out_gitlink_in_a_folder_named_like_a_git_dir_counts_as_unread() {
    for (n, name) in ["git~1", "GIT~1", ".git.", ".g\u{200c}it"].into_iter().enumerate() {
        let dir = one_commit(&format!("gitmodules-folder-{n}"));
        let inner = dir.join(name);
        std::fs::create_dir_all(&inner).unwrap();
        init_repo(&inner);
        write(&inner, "n.txt", "one\n");
        git(&inner, &["add", "."]);
        git(&inner, &["commit", "-q", "-m", "inner"]);
        let repo = git2::Repository::open(&dir).unwrap();
        let head = git2::Repository::open(&inner).unwrap().head().unwrap().target().unwrap().to_string();
        let info = format!("160000,{head},{name}");
        git(&dir, &["-c", "core.protectNTFS=false", "-c", "core.protectHFS=false", "update-index", "--add", "--cacheinfo", &info]);
        assert!(crate::git_file::submodules_unread(&repo), "{name:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
