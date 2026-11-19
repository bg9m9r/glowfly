//! Names and paths that aren't UTF-8: a hooks folder, a program's path, an include and a
//! submodule's folder are read by their bytes where a path may hold any (Unix), and where they
//! can't name a file (Windows) they are named in `TrustScan::unread` or follow the whole
//! working tree, never dropped. The tests that make such names on disk run on Linux only (APFS
//! and HFS+ refuse a name that isn't UTF-8); what holds on every system is checked apart.

use super::*;
use crate::testutil::*;

/// Appends `bytes` to the repository's own config file as written.
fn add_config(dir: &Path, bytes: &[u8]) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(".git/config")).unwrap();
    f.write_all(bytes).unwrap();
}

#[test]
fn a_bare_word_that_isn_t_utf8_names_nothing_to_follow_or_hold() {
    let dir = scratch_repo("bytes-bare");
    add_config(&dir, b"[filter \"x\"]\n\tclean = sh \xff.sh --fast\n");
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.config.len(), 1, "listed: {:?}", s.config);
    assert!(s.follows.is_empty() && s.unread.is_empty(), "{:?} {:?}", s.follows, s.unread);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(unix))]
#[test]
fn a_path_that_names_no_file_here_is_named_unread() {
    let dir = scratch_repo("bytes-unread");
    add_config(&dir, b"[core]\n\thooksPath = hooks-\xff\n\tfsmonitor = ./tools/\xfe.sh\n");
    let s = scan_at(&dir).unwrap();
    assert!(s.config.iter().any(|c| c.key == "core.hooksPath"), "{:?}", s.config);
    assert_eq!(s.hooks_dir, None);
    assert_eq!(s.unread, ["core.hooksPath".to_owned(), "./tools/\u{FFFD}FE.sh".to_owned()]);
    // Never part of the print: a decision can't cover what wasn't read.
    let mut seen = s.clone();
    seen.unread.clear();
    assert_eq!(seen.print(), s.print());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(unix))]
#[test]
fn a_submodule_whose_folder_names_no_folder_here_is_named_unread() {
    let dir = scratch_repo("bytes-gitlink-unread");
    let repo = Repository::open(&dir).unwrap();
    add_gitlink(&repo, b"sub\xff", git2::Oid::hash_object(git2::ObjectType::Commit, b"x").unwrap());
    assert_eq!(scan(&repo).unread, ["sub\u{FFFD}FF".to_owned()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(any(not(unix), target_os = "linux"))]
/// Adds a gitlink at `path` (bytes) to `repo`'s index, pointing at the commit `head`.
fn add_gitlink(repo: &Repository, path: &[u8], head: git2::Oid) {
    let entry = git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o160000,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: head,
        flags: path.len().min(0xfff) as u16,
        flags_extended: 0,
        path: path.to_vec(),
    };
    let mut index = repo.index().unwrap();
    index.add(&entry).unwrap();
    index.write().unwrap();
}

#[cfg(target_os = "linux")]
mod unix {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    fn named(bytes: &[u8]) -> &Path {
        Path::new(OsStr::from_bytes(bytes))
    }

    fn paths(s: &TrustScan) -> Vec<&str> {
        s.follows.iter().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn an_include_whose_path_isn_t_utf8_follows_the_working_tree() {
        let dir = scratch_repo("bytes-include");
        add_config(&dir, b"[include]\n\tpath = ../shared-\xff.cfg\n");
        let s = scan_at(&dir).unwrap();
        assert!(s.config.iter().any(|c| c.key == "include.path"), "listed: {:?}", s.config);
        // Its name is cut where it stops being UTF-8: the version of the whole tree is followed.
        assert!(paths(&s).contains(&""), "{:?}", s.follows);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hooks_folder_whose_name_isn_t_utf8_is_listed_and_read() {
        let dir = scratch_repo("bytes-hooks");
        super::super::tests::hook(&dir.join(named(b"hooks-\xff")), "pre-commit");
        add_config(&dir, b"[core]\n\thooksPath = hooks-\xff\n");
        let s = scan_at(&dir).unwrap();
        assert_eq!(s.hooks, ["pre-commit"]);
        assert!(s.unread.is_empty(), "{:?}", s.unread);
        let before = s.print();
        std::fs::write(dir.join(named(b"hooks-\xff/pre-commit")), "#!/bin/sh\necho two\n").unwrap();
        assert!(!before.covers(&scan_at(&dir).unwrap().print()), "an edited hook asks again");
        // In the working tree its name is cut where it stops being UTF-8: the folder above (the
        // whole tree) is followed.
        assert!(paths(&s).contains(&""), "{:?}", s.follows);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A repository with a tracked `tools/<0xfe>.sh`.
    fn with_odd_script(name: &str) -> PathBuf {
        let dir = scratch_repo(name);
        std::fs::create_dir_all(dir.join("tools")).unwrap();
        std::fs::write(dir.join(named(b"tools/\xfe.sh")), "#!/bin/sh\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "tools"]);
        dir
    }

    #[test]
    fn a_script_whose_name_isn_t_utf8_is_followed_by_its_folder() {
        let dir = with_odd_script("bytes-script");
        add_config(&dir, b"[core]\n\tfsmonitor = ./tools/\xfe.sh\n");
        let s = scan_at(&dir).unwrap();
        assert_eq!(paths(&s), ["tools"]);
        assert!(s.unread.is_empty(), "{:?}", s.unread);
        let _ = std::fs::remove_dir_all(&dir);
        // A hook calling it is followed the same way.
        let dir = with_odd_script("bytes-script-hook");
        let hooks = dir.join(".git/hooks");
        super::super::tests::hook(&hooks, "pre-push");
        std::fs::write(hooks.join("pre-push"), b"#!/bin/sh\nsh ./tools/\xfe.sh\n").unwrap();
        assert_eq!(paths(&scan_at(&dir).unwrap()), ["tools"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_submodule_whose_folder_isn_t_utf8_is_scanned() {
        let dir = scratch_repo("bytes-gitlink");
        let sub = dir.join(named(b"sub\xff"));
        std::fs::create_dir_all(&sub).unwrap();
        init_repo(&sub);
        std::fs::write(sub.join("a.txt"), "a\n").unwrap();
        git(&sub, &["add", "."]);
        git(&sub, &["commit", "-q", "-m", "a"]);
        git(&sub, &["config", "core.fsmonitor", "./fsm.sh"]);
        let head = Repository::open(&sub).unwrap().head().unwrap().target().unwrap();
        add_gitlink(&Repository::open(&dir).unwrap(), b"sub\xff", head);
        let s = scan(&Repository::open(&dir).unwrap());
        assert_eq!(s.submodules.iter().map(|m| m.path.as_str()).collect::<Vec<_>>(), ["sub\u{FFFD}FF"]);
        assert!(s.unread.is_empty(), "{:?}", s.unread);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
