//! `move_back_cmd` against git itself: it moves a branch nobody has checked out back to its old
//! commit and keeps its upstream settings, and git refuses it for the checked-out branch.

use super::*;
use crate::gitcli::run;
use crate::testkit::git_out;
use crate::testutil::*;

fn rev(dir: &std::path::Path, r: &str) -> String {
    git_out(dir, &["rev-parse", r]).trim().to_owned()
}

#[test]
fn move_back_cmd_names_the_branch_and_the_commit() {
    let from = git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert_eq!(move_back_cmd("feature", from), vec!["branch", "-f", "--", "feature", "0123456789abcdef0123456789abcdef01234567"]);
}

#[test]
fn move_back_cmd_resets_a_branch_that_is_not_checked_out_and_keeps_its_upstream() {
    let dir = scratch_repo("move-back-cmd");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "--", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "A"]);
    let a = rev(&dir, "HEAD");
    git(&dir, &["branch", "feature", &a]);
    set_config(&dir, &[("branch.feature.remote", "origin"), ("branch.feature.merge", "refs/heads/feature")]);
    let b = git_out(&dir, &["commit-tree", "HEAD^{tree}", "-p", &a, "-m", "B"]).trim().to_owned();
    git(&dir, &["update-ref", "refs/heads/feature", &b, &a]);
    assert_eq!(rev(&dir, "refs/heads/feature"), b);

    let from = git2::Oid::from_str(&a).unwrap();
    let (ok, out) = run(&dir, &move_back_cmd("feature", from));
    assert!(ok, "{out}");
    assert_eq!(rev(&dir, "refs/heads/feature"), a);
    assert_eq!(git_out(&dir, &["config", "branch.feature.remote"]).trim(), "origin");
    assert_eq!(git_out(&dir, &["config", "branch.feature.merge"]).trim(), "refs/heads/feature");
}

#[test]
fn move_back_cmd_is_refused_for_the_checked_out_branch() {
    let dir = scratch_repo("move-back-checked-out");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "--", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "A"]);
    let a = rev(&dir, "HEAD");
    git(&dir, &["branch", "feature", &a]);
    let b = git_out(&dir, &["commit-tree", "HEAD^{tree}", "-p", &a, "-m", "B"]).trim().to_owned();
    git(&dir, &["update-ref", "refs/heads/feature", &b, &a]);
    git(&dir, &["checkout", "-q", "feature"]);

    let (ok, _) = run(&dir, &move_back_cmd("feature", git2::Oid::from_str(&a).unwrap()));
    assert!(!ok);
    assert_eq!(rev(&dir, "refs/heads/feature"), b);
}
