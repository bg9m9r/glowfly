//! History edits chosen in the graph: the requests they prepare, the refusals, and running them.

use super::plan_tests::{commit, head, linear};
use super::run_tests::{log, message, start_copy};
use super::*;
use crate::error::Error;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn prepared(dir: &Path, edit: QuickEdit) -> QuickPlan {
    prepare_edit(dir, &edit).unwrap_or_else(|e| panic!("{edit:?} was refused: {e}"))
}

fn refused(dir: &Path, edit: QuickEdit) -> String {
    match prepare_edit(dir, &edit) {
        Err(Error::Unsupported(text)) => text,
        Err(e) => panic!("expected a refusal, got {e:?}"),
        Ok(p) => panic!("expected a refusal, got {p:?}"),
    }
}

fn rows(p: &QuickPlan) -> Vec<(Oid, RebaseAction)> {
    p.request.items.iter().filter_map(TodoStep::commit).map(|i| (i.oid, i.action)).collect()
}

#[test]
fn reword_prepares_one_row() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-quick-reword");
    let p = prepared(&dir, QuickEdit::Reword(c2));
    assert_eq!(p.request.upstream, Upstream::Commit(c1));
    assert_eq!(rows(&p), [(c2, RebaseAction::Reword), (c3, RebaseAction::Pick), (c4, RebaseAction::Pick)]);
    assert_eq!((p.edited.as_slice(), p.later, p.branch.as_str()), ([0].as_slice(), 2, "main"));
    assert_eq!(p.request.items[0].commit().unwrap().summary, "c2");
    assert!(!p.request.autostash && p.request.items[0].commit().unwrap().new_message.is_none());

    let p = prepared(&dir, QuickEdit::Reword(c4));
    assert_eq!((rows(&p), p.later), (vec![(c4, RebaseAction::Reword)], 0));
    let p = prepared(&dir, QuickEdit::Reword(c1));
    assert_eq!(p.request.upstream, Upstream::Root);
    assert_eq!(p.later, 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn squash_into_parent_runs() {
    let (dir, [_, c2, c3, c4]) = linear("rebase-quick-squash-parent");
    let p = prepared(&dir, QuickEdit::SquashIntoParent(c3));
    assert_eq!(rows(&p), [(c2, RebaseAction::Reword), (c3, RebaseAction::Fixup), (c4, RebaseAction::Pick)]);
    assert_eq!(p.request.items[0].commit().unwrap().new_message.as_deref(), Some("c2\n\nThe second commit.\n\nc3"));
    assert_eq!((p.edited.as_slice(), p.later), ([0, 1].as_slice(), 1));
    let old_tree = Repository::open(&dir).unwrap().find_commit(c4).unwrap().tree_id();
    let outcome = start_copy(&dir, &p.request);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(log(&dir), ["c4", "c2", "c1"]);
    assert_eq!(message(&dir, "HEAD~1"), "c2\n\nThe second commit.\n\nc3");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().tree_id(), old_tree);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn squash_many_and_not_consecutive() {
    let dir = scratch_repo("rebase-quick-squash-many");
    let c1 = commit(&dir, "1.txt", "1\n", "c1");
    let c2 = commit(&dir, "2.txt", "2\n", "c2");
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    let c4 = commit(&dir, "4.txt", "4\n", "c4");
    assert_eq!(refused(&dir, QuickEdit::Squash(vec![c2, c4])), NOT_CONSECUTIVE);
    assert_eq!(refused(&dir, QuickEdit::Squash(vec![c2])), TOO_FEW_TO_SQUASH);
    let p = prepared(&dir, QuickEdit::Squash(vec![c4, c2, c3]));
    assert_eq!(rows(&p), [(c2, RebaseAction::Reword), (c3, RebaseAction::Fixup), (c4, RebaseAction::Fixup)]);
    assert_eq!(p.request.items[0].commit().unwrap().new_message.as_deref(), Some("c2\n\nc3\n\nc4"));
    assert_eq!(p.request.upstream, Upstream::Commit(c1));
    assert!(matches!(start_copy(&dir, &p.request), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c2", "c1"]);
    assert_eq!(message(&dir, "HEAD"), "c2\n\nc3\n\nc4");
    assert!(dir.join("3.txt").exists() && dir.join("4.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals() {
    let (dir, [c1, c2, ..]) = linear("rebase-quick-refusals");
    assert_eq!(refused(&dir, QuickEdit::SquashIntoParent(c1)), ROOT_SQUASH);
    assert_eq!(refused(&dir, QuickEdit::Drop(head_chain(&dir))), DROPS_EVERYTHING);

    // A commit only on a side branch.
    git(&dir, &["checkout", "-q", "-b", "side", "HEAD~1"]);
    let side = commit(&dir, "s.txt", "s\n", "side");
    git(&dir, &["checkout", "-q", "main"]);
    assert_eq!(refused(&dir, QuickEdit::Reword(side)), NOT_ON_BRANCH);
    assert_eq!(refused(&dir, QuickEdit::Drop(vec![side, c2])), SOME_NOT_ON_BRANCH);

    // A merge on the branch after c2: the side commit came in through it; the merge itself.
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let merge = head(&dir);
    assert_eq!(refused(&dir, QuickEdit::Reword(side)), SIDE_LINE);
    assert_eq!(refused(&dir, QuickEdit::Drop(vec![side, c2])), SOME_SIDE_LINE);
    assert_eq!(refused(&dir, QuickEdit::Reword(merge)), MERGE_SELECTED);
    assert_eq!(refused(&dir, QuickEdit::Drop(vec![merge, c2])), SOME_MERGE_SELECTED);
    commit(&dir, "after.txt", "after\n", "after");
    assert_eq!(refused(&dir, QuickEdit::SquashIntoParent(head(&dir))), PARENT_MERGE);

    git(&dir, &["checkout", "-q", "--detach"]);
    assert_eq!(refused(&dir, QuickEdit::Reword(c2)), DETACHED);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every commit from HEAD down to the root.
fn head_chain(dir: &Path) -> Vec<Oid> {
    let repo = Repository::open(dir).unwrap();
    let mut at = Some(head(dir));
    let mut out = Vec::new();
    while let Some(o) = at {
        out.push(o);
        at = repo.find_commit(o).unwrap().parent_ids().next();
    }
    out
}

#[test]
fn drop_many_runs() {
    let (dir, [_, c2, c3, c4]) = linear("rebase-quick-drop");
    let p = prepared(&dir, QuickEdit::Drop(vec![c4, c2]));
    assert_eq!(rows(&p), [(c2, RebaseAction::Drop), (c3, RebaseAction::Pick), (c4, RebaseAction::Drop)]);
    assert_eq!((p.edited.as_slice(), p.later), ([0, 2].as_slice(), 0));
    assert!(matches!(start_copy(&dir, &p.request), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c3", "c1"]);
    assert!(!dir.join("2.txt").exists() && !dir.join("4.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

fn parent_count(dir: &Path, rev: &str) -> usize {
    Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().parent_count()
}

#[test]
fn edits_across_a_merge() {
    use super::merges_tests::merged;
    let (dir, f) = merged("rebase-quick-across-merge");
    // c2 starts the side line, so it and c3 can't be squashed from the graph.
    assert_eq!(refused(&dir, QuickEdit::Squash(vec![f.c3, f.c2])), SPLIT_BY_BRANCH);
    assert_eq!(refused(&dir, QuickEdit::SquashIntoParent(f.c3)), SPLIT_BY_BRANCH);
    assert_eq!(refused(&dir, QuickEdit::Drop(vec![f.s1, f.c2])), SOME_SIDE_LINE);
    assert_eq!(refused(&dir, QuickEdit::Reword(f.s2)), SIDE_LINE);
    assert_eq!(refused(&dir, QuickEdit::Reword(f.m)), MERGE_SELECTED);
    assert_eq!(refused(&dir, QuickEdit::SquashIntoParent(f.c4)), PARENT_MERGE);

    // A drop before the merge keeps the merge.
    let p = prepared(&dir, QuickEdit::Drop(vec![f.c3]));
    assert_eq!((p.request.upstream, p.later), (Upstream::Commit(f.c2), 2), "the merge and c4 are rewritten");
    assert!(p.request.items.iter().any(|s| s.merge().is_some()), "the list recreates the merge");
    assert!(matches!(start_copy(&dir, &p.request), Outcome::Finished { .. }));
    assert_eq!(parent_count(&dir, "HEAD~1"), 2);
    assert!(!dir.join("3.txt").exists() && dir.join("s2.txt").exists());
    assert!(!log(&dir).contains(&"c3".to_owned()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_squash_before_a_merge_runs() {
    let dir = scratch_repo("rebase-quick-squash-merge");
    commit(&dir, "1.txt", "1\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "s.txt", "s\n", "s1");
    git(&dir, &["checkout", "-q", "main"]);
    let c2 = commit(&dir, "2.txt", "2\n", "c2");
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    commit(&dir, "4.txt", "4\n", "c4");
    let p = prepared(&dir, QuickEdit::Squash(vec![c3, c2]));
    assert_eq!(p.later, 2, "only commit and merge steps count");
    assert!(matches!(start_copy(&dir, &p.request), Outcome::Finished { .. }));
    assert_eq!(parent_count(&dir, "HEAD~1"), 2);
    assert_eq!(message(&dir, "HEAD~2"), "c2\n\nc3");
    assert_eq!(message(&dir, "HEAD~1"), "merge side");
    let _ = std::fs::remove_dir_all(&dir);
}
