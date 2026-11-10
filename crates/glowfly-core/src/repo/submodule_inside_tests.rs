//! Tests for reading a submodule's HEAD from its parent: detached after an update, a branch with
//! its upstream, and nothing for one not checked out.

use super::*;
use crate::testutil::{git, with_submodule};

#[test]
fn an_updated_submodule_is_on_no_branch() {
    let (parent, _) = with_submodule("inside-head-detached");
    // --force checks out the recorded commit even where it already is: a detached HEAD.
    git(&parent, &["submodule", "update", "-q", "--force", "--checkout"]);
    let head = submodule_head(&parent, "libs/lib").unwrap().expect("checked out");
    assert_eq!(head, InsideHead { branch: None, upstream: None });
}

#[test]
fn a_branch_names_its_upstream() {
    let (parent, sub) = with_submodule("inside-head-branch");
    git(&sub, &["checkout", "-q", "-b", "work"]);
    let head = submodule_head(&parent, "libs/lib").unwrap().expect("checked out");
    assert_eq!(head, InsideHead { branch: Some("work".into()), upstream: None });
    git(&sub, &["branch", "-q", "--set-upstream-to=origin/main"]);
    let head = submodule_head(&parent, "libs/lib").unwrap().expect("checked out");
    assert_eq!(head, InsideHead { branch: Some("work".into()), upstream: Some("origin/main".into()) });
}

#[test]
fn an_uninitialized_submodule_has_no_head() {
    let (parent, _) = with_submodule("inside-head-deinit");
    git(&parent, &["submodule", "deinit", "-q", "-f", "libs/lib"]);
    assert_eq!(submodule_head(&parent, "libs/lib").unwrap(), None);
    assert_eq!(submodule_head(&parent, "no/such").unwrap(), None);
}

#[test]
fn short_names_drop_the_ref_prefix() {
    assert_eq!(short_ref("refs/remotes/origin/main"), "origin/main");
    assert_eq!(short_ref("refs/heads/base"), "base");
    assert_eq!(short_ref("odd"), "odd");
}
