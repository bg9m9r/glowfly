//! merge.suppressDest: the defaults, globs that stop at '/', and empty values that clear the
//! globs before them. Read from the fixture's own config so a developer's global config can't
//! change the outcome (branch_ops' merge_ref_tests compare with git itself).

use super::*;
use crate::testutil::{git, scratch_repo};
use std::path::Path;

fn globs(list: &[&str]) -> Vec<String> {
    list.iter().map(|g| (*g).to_owned()).collect()
}

/// `merge_into` for `head` with the merge.suppressDest `values` (None: never set) in a fixture's
/// own config.
fn into(name: &str, values: Option<&[&str]>, head: Option<&str>) -> Option<String> {
    let dir = scratch_repo(name);
    for v in values.unwrap_or_default() {
        git(&dir, &["config", "--add", "merge.suppressDest", v]);
    }
    let got = local_into(&dir, head);
    let _ = std::fs::remove_dir_all(&dir);
    got
}

fn local_into(dir: &Path, head: Option<&str>) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let cfg = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap().snapshot().unwrap();
    merge_into_with(Some(&cfg), head)
}

#[test]
fn without_the_setting_main_and_master_are_left_out() {
    assert_eq!(into("mdest-default-main", None, Some("main")), None);
    assert_eq!(into("mdest-default-master", None, Some("master")), None);
    assert_eq!(into("mdest-default-trunk", None, Some("trunk")).as_deref(), Some("trunk"));
    assert_eq!(into("mdest-default-detached", None, None).as_deref(), Some("HEAD"));
    assert_eq!(merge_into_with(None, Some("main")), None, "an unreadable config keeps git's defaults");
}

#[test]
fn a_set_glob_replaces_the_defaults() {
    assert_eq!(into("mdest-glob-main", Some(&["tr*"]), Some("main")).as_deref(), Some("main"));
    assert_eq!(into("mdest-glob-trunk", Some(&["tr*"]), Some("trunk")), None);
}

#[test]
fn an_empty_value_clears_the_globs_before_it() {
    assert_eq!(into("mdest-clear-all", Some(&["trunk", ""]), Some("main")).as_deref(), Some("main"));
    assert_eq!(into("mdest-clear-trunk", Some(&["trunk", ""]), Some("trunk")).as_deref(), Some("trunk"));
    assert_eq!(into("mdest-clear-then", Some(&["", "main"]), Some("main")), None);
}

#[test]
fn a_star_stops_at_a_slash() {
    assert!(suppressed(&globs(&["team/*"]), "team/x"));
    assert!(!suppressed(&globs(&["*"]), "team/x"));
    assert!(suppressed(&globs(&["*"]), "trunk"));
    assert!(!suppressed(&globs(&["team/*"]), "team/x/y"));
    assert!(!suppressed(&[], "main"));
}

/// Which of `branches` `merge_names` finds shadowed in the repository at `dir`.
fn shadowed_in(dir: &Path, branches: &[BranchRef]) -> Vec<String> {
    let names = merge_names(&Repository::open(dir).unwrap(), Some("main"), branches);
    let mut got: Vec<String> = names.shadowed.into_iter().collect();
    got.sort();
    got
}

#[test]
fn a_ref_git_looks_up_first_shadows_the_branch() {
    let dir = scratch_repo("mdest-shadow");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "base"]);
    for b in ["x", "y", "z", "v", "ORIG", "origin/w"] {
        git(&dir, &["branch", b]);
    }
    let sha = crate::testkit::git_out(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    git(&dir, &["branch", &sha]);
    for r in ["refs/remotes/origin/x", "refs/remotes/origin/w", "refs/remotes/origin/u", "refs/remotes/v"] {
        git(&dir, &["update-ref", r, "HEAD"]);
    }
    git(&dir, &["tag", "x"]);
    git(&dir, &["update-ref", "refs/y", "HEAD"]);
    std::fs::write(Repository::open(&dir).unwrap().path().join("ORIG"), format!("{sha}\n")).unwrap();
    let local = |n: &str| BranchRef::Local(n.into());
    let remote = |n: &str| BranchRef::Remote(n.into());
    let all = [
        local("x"),
        local("y"),
        local("z"),
        local("ORIG"),
        local("origin/w"),
        local("v"),
        local(&sha),
        remote("origin/x"),
        remote("origin/w"),
        remote("origin/u"),
    ];
    let mut want = vec![
        "refs/heads/x",          // a same-named tag
        "refs/heads/y",          // refs/y
        "refs/heads/ORIG",       // an all-caps file in the git dir
        "refs/remotes/origin/w", // a local branch spelled like it
    ];
    let hex = format!("refs/heads/{sha}"); // a full object id
    want.push(&hex);
    want.sort();
    assert_eq!(shadowed_in(&dir, &all), want, "z, v (refs/remotes/v comes after refs/heads/v), origin/x and origin/u are their own");
    let _ = std::fs::remove_dir_all(&dir);
}
