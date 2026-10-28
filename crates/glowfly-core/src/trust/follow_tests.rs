//! Hooks that follow the working tree: found where `core.hooksPath` or a hook's link points
//! into it, part of the print with the version HEAD tracks, and an incoming commit's version
//! read from its objects, so a pull or checkout that brings other hooks is noticed first.

use super::super::incoming::{Target, allows, resolve, targets};
use super::super::tests::hook;
use super::*;
use crate::settings::SavedTrust;
use crate::testutil::*;
use git2::Repository;

fn scan(dir: &Path) -> TrustScan {
    super::super::scan_now(&Repository::open(dir).unwrap())
}

/// A repository whose `core.hooksPath` is the tracked folder `.githooks`, with a post-merge
/// hook committed there.
fn tracked_hooks(name: &str) -> (PathBuf, PathBuf) {
    let (work, bare) = with_origin(name);
    hook(&work.join(".githooks"), "post-merge");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "hooks"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    (work, bare)
}

fn trusted(dir: &Path) -> SavedTrust {
    SavedTrust::trusted(Some(scan(dir).print()))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_hooks_folder_in_the_working_tree_follows_the_branch() {
    let (work, bare) = tracked_hooks("follow-tracked");
    let s = scan(&work);
    assert_eq!(s.hooks, ["post-merge"]);
    let repo = Repository::open(&work).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let id = tree.get_name(".githooks").unwrap().id();
    assert_eq!(s.follows, [Follow { path: ".githooks".into(), tracked: Some((".githooks".into(), id)), hooks: true }]);
    // Committing another version of the folder is another print: it asks again.
    let before = s.print();
    std::fs::write(work.join(".githooks/post-merge"), "#!/bin/sh\necho two\n").unwrap();
    git(&work, &["commit", "-q", "-am", "edit"]);
    let after = scan(&work);
    assert!(!before.covers(&after.print()));
    assert_eq!(after.gained_since(&before).follows.len(), 1);
    cleanup(&[&work, &bare]);
}

#[test]
fn hooks_in_the_git_dir_follow_nothing() {
    let dir = scratch_repo("follow-git-dir");
    hook(&dir.join(".git/hooks"), "post-checkout");
    assert!(scan(&dir).follows.is_empty());
    // Pointing core.hooksPath at the git dir's own folder is still the git dir.
    git(&dir, &["config", "core.hooksPath", ".git/hooks"]);
    assert!(scan(&dir).follows.is_empty());
    cleanup(&[&dir]);
}

#[test]
fn an_untracked_hooks_folder_is_covered_by_the_tracked_folder_around_it() {
    // husky: `.husky/_` is made by `husky install` and ignored; the scripts its hooks run
    // (`.husky/pre-commit`) are tracked beside it.
    let dir = scratch_repo("follow-husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    write(&dir, ".husky/pre-commit", "npm test\n");
    write(&dir, ".gitignore", ".husky/_\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "husky"]);
    hook(&dir.join(".husky/_"), "pre-commit");
    git(&dir, &["config", "core.hooksPath", ".husky/_"]);
    let s = scan(&dir);
    let f = &s.follows[0];
    assert_eq!((f.path.as_str(), f.tracked.as_ref().map(|t| t.0.as_str())), (".husky/_", Some(".husky")));
    // A folder nothing above tracks still follows, with no version: a branch that adds it is new.
    git(&dir, &["config", "core.hooksPath", "local-hooks"]);
    assert_eq!(scan(&dir).follows, [Follow { path: "local-hooks".into(), tracked: None, hooks: true }]);
    cleanup(&[&dir]);
}

#[test]
fn a_tracked_hooks_folder_changed_by_a_pull_is_not_covered() {
    let (work, bare) = tracked_hooks("follow-pull");
    let saved = trusted(&work);
    let pull = targets(&crate::gitcli::args(&["pull", "--ff"]));
    let Some(Target::Fetched { fetch, upstream: true }) = pull.first().cloned() else { panic!("{pull:?}") };
    assert_eq!(fetch, ["fetch"]);
    let incoming = |work: &Path| {
        git(work, &fetch.iter().map(String::as_str).collect::<Vec<_>>());
        let repo = Repository::open(work).unwrap();
        let oid = resolve(&repo, &pull[0]).expect("the upstream");
        incoming(&repo, &scan(work).follows, oid).unwrap()
    };
    // Something else changed upstream: the hooks the pull brings are the trusted ones.
    remote_commit(&bare, "main", "b.txt", "other\n");
    assert!(allows(Some(&saved), &incoming(&work)), "same hooks");
    // The hook changed upstream: held before git runs it as post-merge.
    remote_commit(&bare, "main", ".githooks/post-merge", "#!/bin/sh\necho changed\n");
    let inc = incoming(&work);
    assert!(!allows(Some(&saved), &inc), "{inc:?}");
    assert_eq!(inc.hooks, ["post-merge"], "its hooks are read from the commit");
    // Trusting it covers it, and the checked-out hook it becomes.
    let both = SavedTrust::trusted(Some(saved.print.clone().unwrap().with(&inc.print())));
    assert!(allows(Some(&both), &inc));
    git(&work, &["-c", "core.hooksPath=.git/hooks", "merge", "-q", "--ff-only", "@{u}"]);
    assert!(both.print.as_ref().unwrap().covers(&scan(&work).print()), "no second question once it lands");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_checkout_of_another_branch_compares_its_hooks() {
    let (work, bare) = tracked_hooks("follow-checkout");
    let saved = trusted(&work);
    git(&work, &["checkout", "-q", "-b", "side"]);
    std::fs::write(work.join(".githooks/post-checkout"), "#!/bin/sh\necho hi\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["-c", "core.hooksPath=.git/hooks", "commit", "-q", "-m", "add"]);
    git(&work, &["-c", "core.hooksPath=.git/hooks", "checkout", "-q", "main"]);
    let repo = Repository::open(&work).unwrap();
    let follows = scan(&work).follows;
    let check = |args: &[&str]| {
        targets(&crate::gitcli::args(args))
            .iter()
            .filter_map(|t| resolve(&repo, t))
            .all(|oid| allows(Some(&saved), &incoming(&repo, &follows, oid).unwrap()))
    };
    assert!(!check(&["checkout", "side", "--"]), "side adds a hook");
    assert!(!check(&["merge", "--no-ff", "side"]));
    assert!(check(&["checkout", "main", "--"]), "the branch already trusted");
    assert!(check(&["checkout", "-b", "new"]), "a new branch at HEAD");
    cleanup(&[&work, &bare]);
}

#[cfg(unix)]
#[test]
fn a_hook_linked_into_the_working_tree_follows_it() {
    let dir = scratch_repo("follow-link");
    hook(&dir.join("scripts"), "check.sh");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "scripts"]);
    std::os::unix::fs::symlink(dir.join("scripts/check.sh"), dir.join(".git/hooks/pre-commit")).unwrap();
    let f = &scan(&dir).follows;
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!((f[0].path.as_str(), f[0].hooks), ("scripts/check.sh", false));
    cleanup(&[&dir]);
}
