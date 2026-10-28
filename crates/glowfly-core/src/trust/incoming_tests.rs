//! Reading what a command brings into the working tree from git's arguments, and resolving it.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;

fn of(a: &[&str]) -> Vec<Target> {
    targets(&args(a))
}

fn rev(s: &str) -> Target {
    Target::Rev(s.to_owned())
}

#[test]
fn the_commands_that_move_the_working_tree_name_their_commit() {
    assert_eq!(of(&["checkout", "main", "--"]), [rev("main")]);
    assert_eq!(of(&["checkout", "--track", "origin/x", "--"]), [rev("origin/x")]);
    assert_eq!(of(&["checkout", "-b", "new", "origin/x"]), [rev("origin/x")]);
    assert_eq!(of(&["checkout", "-b", "new"]), [], "a new branch at HEAD");
    assert_eq!(of(&["checkout", "--", "a.txt"]), [], "files from the index");
    assert_eq!(of(&["switch", "-c", "new", "v1"]), [rev("v1")]);
    assert_eq!(of(&["checkout", "-"]), [rev("@{-1}")]);
    assert_eq!(of(&["merge", "--no-ff", "-m", "msg", "topic", "other"]), [rev("topic"), rev("other")]);
    assert_eq!(of(&["merge", "--abort"]), []);
    assert_eq!(of(&["rebase", "main"]), [rev("main")]);
    assert_eq!(of(&["rebase", "--onto", "v2", "v1"]), [rev("v2")]);
    assert_eq!(of(&["rebase", "--onto=v2", "v1", "topic"]), [rev("v2")]);
    assert_eq!(of(&["rebase"]), [Target::Upstream]);
    assert_eq!(of(&["rebase", "--root"]), []);
    assert_eq!(of(&["rebase", "--continue"]), []);
    assert_eq!(of(&["-c", "x=y", "pull", "--rebase"]), [Target::Fetched { fetch: args(&["-c", "x=y", "fetch"]), upstream: true }]);
    assert_eq!(of(&["pull", "origin", "main"]), [Target::Fetched { fetch: args(&["fetch", "origin", "main"]), upstream: false }]);
    assert_eq!(of(&["commit", "-m", "x"]), []);
}

#[test]
fn a_branch_only_one_remote_has_resolves_as_checkout_makes_it() {
    let (work, bare) = with_origin("incoming-dwim");
    remote_commit(&bare, "main", "b.txt", "b\n");
    git(&work, &["fetch", "-q"]);
    git(&work, &["branch", "-q", "-r", "-D", "origin/main"]);
    git(&work, &["fetch", "-q", "origin", "main:refs/remotes/origin/only"]);
    let repo = Repository::open(&work).unwrap();
    let only = repo.revparse_single("origin/only").unwrap().id();
    assert_eq!(resolve(&repo, &rev("only")), Some(only));
    assert_eq!(resolve(&repo, &rev("nowhere")), None);
    assert_eq!(resolve(&repo, &rev("main")), Some(repo.head().unwrap().target().unwrap()));
    // FETCH_HEAD's commit for merging, as a pull with a remote named merges it.
    let fetched = Target::Fetched { fetch: Vec::new(), upstream: false };
    assert_eq!(resolve(&repo, &fetched), Some(only));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}
