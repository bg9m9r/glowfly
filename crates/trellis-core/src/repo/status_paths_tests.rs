//! Re-reading some work-tree paths, merged into the lists a whole read gave before, must give
//! exactly what a whole read gives now: in libgit2's mode and in git's (untracked cache on).

use super::super::bytes_tests::blob;
use super::*;
use crate::gitcli::{args, run};
use crate::repo::{merge_unstaged, status_at};
use crate::testutil::*;
use std::path::{Path, PathBuf};

type Lists = (Vec<FileEntry>, Vec<FileEntry>);

fn full(dir: &Path, git_status: bool) -> Lists {
    let (unstaged, staged, _) = status_at(dir, git_status).unwrap();
    (unstaged, staged)
}

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

/// `before` with the unstaged rows under `paths` read again, or a whole read when they can't be
/// read on their own.
fn incremental(dir: &Path, before: &Lists, paths: &[&str], git_status: bool) -> Lists {
    match status_of_paths(dir, &owned(paths), git_status).unwrap() {
        Some(ps) => {
            let mut rows = before.0.clone();
            let fold = collide::ignorecase(&Repository::open(dir).unwrap());
            merge_unstaged(&mut rows, &ps.covered, ps.unstaged, fold);
            (rows, before.1.clone())
        }
        None => full(dir, git_status),
    }
}

/// Reads `paths` on their own (they must be readable so) and checks the merge against a whole read.
fn check(dir: &Path, before: &Lists, paths: &[&str], git_status: bool) -> Lists {
    assert!(status_of_paths(dir, &owned(paths), git_status).unwrap().is_some(), "{paths:?} read on their own");
    let merged = incremental(dir, before, paths, git_status);
    assert_eq!(merged, full(dir, git_status), "{paths:?} (git: {git_status})");
    merged
}

fn rows(lists: &[FileEntry]) -> Vec<(&str, char)> {
    lists.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

/// Runs `case` on a fresh repository holding `files` (committed), once with libgit2's read and
/// once with git's own (the untracked cache on, so the status goes through git).
fn both(name: &str, files: &[(&str, &str)], case: impl Fn(&Path, bool)) {
    for git_status in [false, true] {
        let dir = scratch_repo(&format!("{name}-{}", if git_status { "git" } else { "lib" }));
        if git_status {
            set_config(&dir, &[("core.untrackedCache", "true")]);
        }
        commit_files(&dir, files);
        case(&dir, git_status);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn commit_files(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(dir.join(parent)).unwrap();
        }
        write(dir, path, text);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "files"]);
}

#[test]
fn edits_show_and_go() {
    both("paths-edits", &[("a.txt", "one\n"), ("b.txt", "b\n")], |dir, g| {
        let before = full(dir, g);
        write(dir, "a.txt", "two\n");
        let after = check(dir, &before, &["a.txt"], g);
        assert_eq!(rows(&after.0), [("a.txt", 'M')]);
        write(dir, "a.txt", "one\n");
        assert!(check(dir, &after, &["a.txt"], g).0.is_empty(), "restored");
    });
}

#[test]
fn new_files_and_folders() {
    both("paths-new", &[("a.txt", "a\n")], |dir, g| {
        let before = full(dir, g);
        write(dir, "b.txt", "b\n");
        let after = check(dir, &before, &["b.txt"], g);
        std::fs::create_dir_all(dir.join("new/x")).unwrap();
        write(dir, "new/x/y.txt", "y\n");
        write(dir, "new/z.txt", "z\n");
        // Only the folder's own event: everything below it is read.
        let after = check(dir, &after, &["new"], g);
        assert_eq!(rows(&after.0), [("b.txt", '?'), ("new/x/y.txt", '?'), ("new/z.txt", '?')]);
    });
}

#[test]
fn deleted_files_and_folders() {
    both("paths-deleted", &[("a.txt", "a\n"), ("dir/x.txt", "x\n"), ("dir/sub/y.txt", "y\n")], |dir, g| {
        let before = full(dir, g);
        std::fs::remove_file(dir.join("a.txt")).unwrap();
        let after = check(dir, &before, &["a.txt"], g);
        std::fs::remove_dir_all(dir.join("dir")).unwrap();
        let after = check(dir, &after, &["dir"], g);
        assert_eq!(rows(&after.0), [("a.txt", 'D'), ("dir/sub/y.txt", 'D'), ("dir/x.txt", 'D')]);
    });
}

#[test]
fn renames_in_the_work_tree_and_the_index() {
    both("paths-renames", &[("a.txt", "a\n"), ("b.txt", "b\n")], |dir, g| {
        let before = full(dir, g);
        std::fs::rename(dir.join("a.txt"), dir.join("c.txt")).unwrap();
        let after = check(dir, &before, &["a.txt", "c.txt"], g);
        assert_eq!(rows(&after.0), [("a.txt", 'D'), ("c.txt", '?')]);

        std::fs::rename(dir.join("c.txt"), dir.join("a.txt")).unwrap();
        git(dir, &["mv", "b.txt", "d.txt"]);
        // The index changed: a whole read, as the watcher's index event asks for.
        let before = full(dir, g);
        write(dir, "d.txt", "edited\n");
        let after = check(dir, &before, &["d.txt"], g);
        assert_eq!(rows(&after.1), [("d.txt", 'R')], "the staged rename is kept");
        assert_eq!(rows(&after.0), [("d.txt", 'M')]);
    });
}

#[test]
fn an_ignore_or_attributes_file_reads_everything() {
    both("paths-ignored", &[("a.txt", "a\n")], |dir, g| {
        write(dir, "x.log", "log\n");
        let before = full(dir, g);
        assert_eq!(rows(&before.0), [("x.log", '?')]);
        write(dir, ".gitignore", "*.log\n");
        assert_eq!(status_of_paths(dir, &owned(&[".gitignore"]), g).unwrap(), None);
        let after = incremental(dir, &before, &[".gitignore"], g);
        assert_eq!(after, full(dir, g));
        assert_eq!(rows(&after.0), [(".gitignore", '?')], "x.log is ignored now");
        for p in ["sub/.gitignore", ".gitattributes", "a/.gitattributes", ".gitmodules"] {
            assert_eq!(status_of_paths(dir, &owned(&[p]), g).unwrap(), None, "{p}");
        }
    });
}

#[test]
fn staged_files_changed_again() {
    both("paths-staged", &[("a.txt", "a\n")], |dir, g| {
        write(dir, "n.txt", "new\n");
        write(dir, "m.txt", "new too\n");
        git(dir, &["add", "n.txt", "m.txt"]);
        let before = full(dir, g);
        write(dir, "n.txt", "edited\n");
        std::fs::remove_file(dir.join("m.txt")).unwrap();
        let after = check(dir, &before, &["n.txt", "m.txt"], g);
        assert_eq!(rows(&after.1), [("m.txt", 'A'), ("n.txt", 'A')]);
        assert_eq!(rows(&after.0), [("m.txt", 'D'), ("n.txt", 'M')]);
    });
}

#[test]
fn a_conflicted_file_is_listed_once() {
    both("paths-conflict", &[("f.txt", "base\n")], |dir, g| {
        git(dir, &["checkout", "-q", "-b", "other"]);
        commit_files(dir, &[("f.txt", "theirs\n")]);
        git(dir, &["checkout", "-q", "main"]);
        commit_files(dir, &[("f.txt", "ours\n")]);
        let (ok, _) = run(dir, &args(&["merge", "-q", "other"]));
        assert!(!ok, "the merge conflicts");
        let before = full(dir, g);
        write(dir, "f.txt", "resolving\n");
        let after = check(dir, &before, &["f.txt"], g);
        assert_eq!(rows(&after.0), [("f.txt", '!')]);
    });
}

#[test]
fn files_a_sparse_checkout_leaves_out_stay_quiet() {
    let dir = scratch_repo("paths-sparse");
    commit_files(&dir, &[("r.txt", "r\n"), ("src/x.txt", "x\n"), ("docs/d.txt", "d\n")]);
    git(&dir, &["sparse-checkout", "set", "--cone", "--no-sparse-index", "src"]);
    assert!(!dir.join("docs/d.txt").exists());
    let before = full(&dir, false);
    let after = check(&dir, &before, &["docs/d.txt"], false);
    assert!(after.0.is_empty(), "{:?}", after.0);
    assert!(check(&dir, &before, &["docs"], false).0.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twins_are_marked_as_a_whole_read_marks_them() {
    let dir = scratch_repo("paths-twins");
    git(&dir, &["config", "core.ignorecase", "false"]);
    commit_files(&dir, &[("other.txt", "other\n")]);
    for p in ["a.txt", "A.txt"] {
        let id = blob(&dir, &format!("{p}\n"));
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{id},{p}")]);
    }
    git(&dir, &["commit", "-q", "-m", "twins"]);
    write(&dir, "a.txt", "a.txt\n");
    let before = full(&dir, false);
    write(&dir, "a.txt", "edited\n");
    // The name on disk only; on a volume that folds case it is also A.txt's file.
    let after = check(&dir, &before, &["a.txt"], false);
    let a = after.0.iter().find(|r| r.path == "a.txt").expect("a.txt changed");
    if collide::folding(Repository::open(&dir).unwrap().path()).case != Some(false) {
        assert!(matches!(&a.hazard, Some(super::super::Hazard::Twins(t)) if t == &["A.txt".to_string()]), "{:?}", a.hazard);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_inside_a_submodule_reads_its_row() {
    for git_status in [false, true] {
        let name = format!("paths-sm-{}", if git_status { "git" } else { "lib" });
        let (parent, sub) = with_submodule(&name);
        if git_status {
            set_config(&parent, &[("core.untrackedCache", "true")]);
        }
        let before = full(&parent, git_status);
        write(&sub, "lib.txt", "changed\n");
        let ps = status_of_paths(&parent, &owned(&["libs/lib/lib.txt"]), git_status).unwrap().expect("read on its own");
        assert_eq!(ps.covered, ["libs/lib"]);
        assert!(ps.submodule_touched);
        let after = check(&parent, &before, &["libs/lib/lib.txt"], git_status);
        assert_eq!(rows(&after.0), [("libs/lib", 'M')]);
        let lib = parent.with_file_name(format!("trellis-test-{name}-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&parent);
    }
}

#[test]
fn a_nested_repository_is_read_from_its_root() {
    let dir = scratch_repo("paths-nested");
    commit_files(&dir, &[("a.txt", "a\n")]);
    let nested = dir.join("vendor/inner");
    std::fs::create_dir_all(&nested).unwrap();
    git(&nested, &["init", "-q"]);
    write(&nested, "x.txt", "x\n");
    let ps = status_of_paths(&dir, &owned(&["vendor/inner/x.txt"]), false).unwrap().expect("read on its own");
    assert_eq!(ps.covered, ["vendor/inner"]);
    assert!(!ps.submodule_touched);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn names_that_look_like_patterns_are_literal() {
    both("paths-literal", &[("a.txt", "a\n"), ("[ab].txt", "glob\n")], |dir, g| {
        write(dir, "a.txt", "edited\n");
        let before = full(dir, g);
        write(dir, "[ab].txt", "edited\n");
        let ps = status_of_paths(dir, &owned(&["[ab].txt"]), g).unwrap().expect("read on its own");
        assert_eq!(rows(&ps.unstaged), [("[ab].txt", 'M')], "a.txt isn't matched");
        let after = check(dir, &before, &["[ab].txt"], g);
        assert_eq!(rows(&after.0), [("[ab].txt", 'M'), ("a.txt", 'M')]);
    });
}

/// A path that is absolute on every OS.
fn absolute() -> String {
    let p: PathBuf = crate::paths::temp_dir().join("x.txt");
    assert!(p.is_absolute());
    p.to_string_lossy().replace('\\', "/")
}

#[test]
fn paths_that_cannot_be_read_alone_read_everything() {
    let dir = scratch_repo("paths-rejects");
    commit_files(&dir, &[("a.txt", "a\n")]);
    let none = |paths: Vec<String>, g: bool| status_of_paths(&dir, &paths, g).unwrap().is_none();
    assert!(none(Vec::new(), false));
    for bad in ["../x", "a//b", "", "./a.txt", "a/./b", "a/..", "/a.txt", "a/", absolute().as_str()] {
        assert!(none(vec![bad.to_owned()], false), "{bad:?}");
    }
    let many: Vec<String> = (0..=crate::watch::MAX_WATCH_PATHS).map(|i| format!("f{i}.txt")).collect();
    assert!(none(many.clone(), false), "more than MAX_WATCH_PATHS");
    assert!(!none(many[..crate::watch::MAX_WATCH_PATHS].to_vec(), false), "up to the limit is fine");
    // Long names: fine for libgit2, too long for one git command line.
    let long: Vec<String> = (0..20).map(|i| format!("{}{i}", "n".repeat(1000))).collect();
    assert!(!none(long.clone(), false));
    set_config(&dir, &[("core.untrackedCache", "true")]);
    assert!(none(long, true), "over CLI_PATH_BYTES");
    assert!(!none(owned(&["a.txt"]), true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn covered_paths_are_sorted_and_minimal() {
    let dir = scratch_repo("paths-normal");
    commit_files(&dir, &[("a/b/c.txt", "c\n")]);
    let ps = status_of_paths(&dir, &owned(&["a/b", "a", "a", "z", "a/b/c.txt"]), false).unwrap().unwrap();
    assert_eq!(ps.covered, ["a", "z"]);
    let mut p = owned(&["b/c", "ab", "a", "b", "a/x"]);
    normalize(&mut p);
    assert_eq!(p, ["a", "ab", "b"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn requested_paths_are_composed_under_precomposeunicode() {
    let dir = scratch_repo("paths-nfc");
    commit_files(&dir, &[("a.txt", "a\n")]);
    set_config(&dir, &[("core.precomposeunicode", "true")]);
    let ps = status_of_paths(&dir, &owned(&["cafe\u{301}.txt"]), false).unwrap().unwrap();
    assert_eq!(ps.covered, ["caf\u{e9}.txt"]);
    set_config(&dir, &[("core.precomposeunicode", "false")]);
    let ps = status_of_paths(&dir, &owned(&["cafe\u{301}.txt"]), false).unwrap().unwrap();
    assert_eq!(ps.covered, ["cafe\u{301}.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}
