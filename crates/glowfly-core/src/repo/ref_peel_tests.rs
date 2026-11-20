//! `Peeler`: what a ref's target is, from object headers, and that each target is read once.

use super::*;
use crate::testutil::*;

#[test]
fn targets_peel_to_commits_without_reading_them() {
    let dir = scratch_repo("ref-peel");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "c"]);
    git(&dir, &["tag", "-a", "inner", "-m", "i"]);
    git(&dir, &["tag", "-a", "outer", "-m", "o", "inner"]);
    let repo = Repository::open(&dir).unwrap();
    let id = |spec: &str| repo.revparse_single(spec).unwrap().id();
    let (commit, tree, inner, outer) = (id("HEAD"), id("HEAD^{tree}"), id("refs/tags/inner"), id("refs/tags/outer"));
    let blob = repo.blob(b"text").unwrap();
    let mut peeler = Peeler::new(&repo);
    assert_eq!(peeler.peel(commit), Peeled::Commit(commit));
    assert_eq!(peeler.peel(inner), Peeled::Tag(commit));
    assert_eq!(peeler.peel(outer), Peeled::Tag(commit), "a tag of a tag");
    assert_eq!(peeler.peel(tree), Peeled::Other);
    assert_eq!(peeler.peel(blob), Peeled::Other);
    assert_eq!(peeler.peel(Oid::zero()), Peeled::Other, "no such object");
    let missing = Oid::from_str("0123456789012345678901234567890123456789").unwrap();
    assert_eq!(peeler.peel(missing), Peeled::Other);
    assert_eq!((Peeled::Commit(commit).commit(), Peeled::Tag(commit).commit(), Peeled::Other.commit()), (Some(commit), Some(commit), None));
    let known = peeler.memo.len();
    assert_eq!(peeler.peel(outer), Peeled::Tag(commit));
    assert_eq!(peeler.peel(commit), Peeled::Commit(commit));
    assert_eq!(peeler.memo.len(), known, "a second ask is answered from the memo");
}

#[test]
fn a_tag_of_a_missing_commit_or_a_tree_is_other() {
    let dir = scratch_repo("ref-peel-other");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "c"]);
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.revparse_single("HEAD^{tree}").unwrap();
    let sig = repo.signature().unwrap();
    let on_tree = repo.tag("on-tree", &tree, &sig, "a tag on a tree", false).unwrap();
    let mut peeler = Peeler::new(&repo);
    assert_eq!(peeler.peel(on_tree), Peeled::Other);
}
