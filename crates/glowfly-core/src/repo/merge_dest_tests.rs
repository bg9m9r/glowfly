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
