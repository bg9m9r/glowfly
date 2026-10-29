//! A merge of two trusted versions of a hooks folder that makes a third is held; a conflict in
//! it is held as Conflicted.

use super::*;
use crate::testutil::*;
use std::path::Path;

/// A repository whose tracked `.githooks/post-merge` has ten lines, `core.hooksPath` set to it,
/// and a branch `side` that changes its last line while `main` changes `first`'s line one.
fn diverged(name: &str, first: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    let lines: Vec<String> = (1..=10).map(|n| format!("echo {n}")).collect();
    let write = |text: &str| {
        std::fs::create_dir_all(dir.join(".githooks")).unwrap();
        std::fs::write(dir.join(".githooks/post-merge"), text).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "hook"]);
    };
    write(&format!("{}\n", lines.join("\n")));
    git(&dir, &["branch", "side"]);
    let mut mine = lines.clone();
    mine[0] = first.to_owned();
    write(&format!("{}\n", mine.join("\n")));
    git(&dir, &["checkout", "-q", "side"]);
    let mut theirs = lines;
    theirs[9] = "echo theirs".to_owned();
    write(&format!("{}\n", theirs.join("\n")));
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["config", "core.hooksPath", ".githooks"]);
    dir
}

/// Trusted as HEAD is and with `side`'s version too.
fn both_trusted(repo: &Repository) -> (Vec<Follow>, SavedTrust) {
    let now = super::super::scan_now(repo);
    let side = repo.revparse_single("side").unwrap().id();
    let theirs = TrustScan { follows: follow::versions(repo, &now.follows, side).unwrap(), ..TrustScan::default() };
    let mut print = now.print();
    print.0.extend(theirs.print().0);
    (now.follows, SavedTrust::trusted(Some(print)))
}

fn merge_side(repo: &Repository) -> Vec<Step> {
    steps(repo, &Target::Merged(vec!["side".into()])).unwrap()
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_merge_of_two_trusted_versions_that_makes_a_third_is_held() {
    let dir = diverged("merged-third", "echo mine");
    let repo = Repository::open(&dir).unwrap();
    let (follows, saved) = both_trusted(&repo);
    // Each side alone is trusted.
    let side = repo.revparse_single("side").unwrap().id();
    assert_eq!(super::super::compare::check(&repo, &follows, Some(&saved), &[side]), Ok(()));
    match check(&repo, &follows, Some(&saved), &merge_side(&repo)) {
        Err(Refused::Held(found)) => assert_eq!(found.hooks, ["post-merge"]),
        other => panic!("held: {other:?}"),
    }
    // A cherry-pick of side's commit makes the same third version.
    let picks = steps(&repo, &Target::Picked { revs: vec!["side".into()], revert: false }).unwrap();
    assert!(matches!(check(&repo, &follows, Some(&saved), &picks), Err(Refused::Held(_))));
    cleanup(&dir);
}

#[test]
fn a_merge_that_conflicts_in_the_hooks_folder_is_held_as_conflicted() {
    let dir = diverged("merged-conflict", "echo mine");
    let repo = Repository::open(&dir).unwrap();
    // Both change line ten: change main's again so they conflict.
    let text = std::fs::read_to_string(dir.join(".githooks/post-merge")).unwrap().replace("echo 10", "echo ours");
    std::fs::write(dir.join(".githooks/post-merge"), text).unwrap();
    git(&dir, &["commit", "-q", "-am", "mine"]);
    let (follows, saved) = both_trusted(&repo);
    let got = check(&repo, &follows, Some(&saved), &merge_side(&repo));
    assert_eq!(got, Err(Refused::Unchecked(Unchecked::Conflicted(".githooks/post-merge".into()))));
    cleanup(&dir);
}

#[test]
fn a_merge_that_leaves_the_hooks_as_head_has_them_merges_nothing() {
    let dir = scratch_repo("merged-same");
    std::fs::create_dir_all(dir.join(".githooks")).unwrap();
    std::fs::write(dir.join(".githooks/post-merge"), "echo\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "hook"]);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["config", "core.hooksPath", ".githooks"]);
    let repo = Repository::open(&dir).unwrap();
    let now = super::super::scan_now(&repo);
    let saved = SavedTrust::trusted(Some(now.print()));
    assert_eq!(check(&repo, &now.follows, Some(&saved), &merge_side(&repo)), Ok(()));
    cleanup(&dir);
}
