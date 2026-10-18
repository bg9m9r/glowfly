use super::*;
use crate::testutil::git;
use std::fs;

/// A fresh, empty folder for one test.
fn fixture(name: &str) -> PathBuf {
    let dir = paths::temp_dir().join(format!("trellis-scan-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A repository made with `git init` at `dir/rel`.
fn init(dir: &Path, rel: &str) -> PathBuf {
    let repo = dir.join(rel);
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    repo
}

fn found(report: &ScanReport) -> Vec<PathBuf> {
    report.found.iter().map(|f| f.path.clone()).collect()
}

#[test]
fn finds_repositories_below_and_at_the_root() {
    let dir = fixture("below");
    let deep = init(&dir, "clients/acme/web");
    fs::create_dir_all(dir.join("notes/plain")).unwrap();
    let report = scan(std::slice::from_ref(&dir));
    assert_eq!(found(&report), std::slice::from_ref(&deep));
    assert_eq!(report.found[0].root, dir);
    assert!(report.unreadable.is_empty() && !report.truncated, "{report:?}");
    // The root itself can be one, and nothing below it is listed then.
    let report = scan(std::slice::from_ref(&deep));
    assert_eq!(found(&report), [deep]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn does_not_descend_into_a_repository() {
    let dir = fixture("nested");
    let outer = init(&dir, "outer");
    init(&dir, "outer/vendor/inner");
    assert_eq!(found(&scan(std::slice::from_ref(&dir))), [outer]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn finds_a_linked_worktree_folder() {
    let dir = fixture("worktree");
    let wt = dir.join("feature");
    fs::create_dir_all(&wt).unwrap();
    fs::write(wt.join(".git"), "gitdir: ../main/.git/worktrees/feature\n").unwrap();
    // A `.git` file that says anything else, and an empty `.git` folder, are not repositories.
    let odd = dir.join("odd");
    fs::create_dir_all(&odd).unwrap();
    fs::write(odd.join(".git"), "nothing here\n").unwrap();
    assert_eq!(found(&scan(std::slice::from_ref(&dir))), [wt]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn is_repo_folder_reads_the_git_entry() {
    let dir = fixture("entry");
    let with_head = dir.join("a");
    fs::create_dir_all(with_head.join(".git")).unwrap();
    assert!(!is_repo_folder(&with_head), "an empty .git folder");
    fs::write(with_head.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    assert!(is_repo_folder(&with_head));
    let file = dir.join("b");
    fs::create_dir_all(&file).unwrap();
    fs::write(file.join(".git"), "gitdir: /elsewhere/.git/worktrees/b\n").unwrap();
    assert!(is_repo_folder(&file));
    let plain = dir.join("c");
    fs::create_dir_all(&plain).unwrap();
    assert!(!is_repo_folder(&plain));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn skips_node_modules_and_git_folders() {
    let dir = fixture("skips");
    init(&dir, "app/node_modules/pkg");
    init(&dir, "NODE_MODULES/other");
    // A `.git` folder without HEAD isn't a repository, and isn't entered either.
    init(&dir, "plain/.git/inner");
    let report = scan(std::slice::from_ref(&dir));
    assert!(report.found.is_empty(), "{report:?}");
    assert!(skipped(OsStr::new("$Recycle.Bin")) && skipped(OsStr::new("system volume information")));
    assert!(!skipped(OsStr::new("node_modules2")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn respects_depth_and_folder_limits() {
    let dir = fixture("limits");
    let four = init(&dir, "a/b/c/d");
    init(&dir, "e/f/g/h/i");
    let report = scan_with(std::slice::from_ref(&dir), 4, MAX_FOLDERS);
    assert_eq!(found(&report), [four], "level 5 is too deep");
    assert!(!report.truncated);
    let report = scan_with(std::slice::from_ref(&dir), 4, 3);
    assert!(report.truncated, "{report:?}");
    assert!(report.found.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_root_is_unreadable() {
    let dir = fixture("missing");
    let gone = dir.join("gone");
    let report = scan(&[gone.clone(), dir.clone()]);
    assert_eq!(report.unreadable.len(), 1, "{report:?}");
    assert_eq!(report.unreadable[0].0, gone);
    assert!(!report.unreadable[0].1.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn overlapping_roots_list_a_repository_once() {
    let dir = fixture("overlap");
    let repo = init(&dir, "team/api");
    let report = scan(&[dir.clone(), dir.join("team")]);
    assert_eq!(found(&report), [repo]);
    assert_eq!(report.found[0].root, dir, "under the first root");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn results_sort_by_name_ignoring_case() {
    let dir = fixture("sorted");
    let zeta = init(&dir, "x/zeta");
    let alpha = init(&dir, "y/Alpha");
    let beta = init(&dir, "beta");
    assert_eq!(found(&scan(std::slice::from_ref(&dir))), [alpha, beta, zeta]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn links_are_not_followed() {
    let dir = fixture("links");
    let outside = fixture("links-target");
    init(&outside, "linked");
    let Some(link) = crate::testutil::alias_of(&outside) else {
        eprintln!("links_are_not_followed: can't make a symlink or junction here, skipping");
        return;
    };
    let inside = dir.join("via");
    // Move the alias into the project folder.
    if fs::rename(&link, &inside).is_err() {
        eprintln!("links_are_not_followed: can't move the link, skipping");
        let _ = fs::remove_dir(&link).or_else(|_| fs::remove_file(&link));
        return;
    }
    assert!(scan(std::slice::from_ref(&dir)).found.is_empty(), "the link was followed");
    let _ = fs::remove_dir(&inside).or_else(|_| fs::remove_file(&inside));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
}
