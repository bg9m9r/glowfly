//! Planning an interactive rebase against fixture repositories.

use super::*;
use crate::error::Error;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

pub(super) fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

pub(super) fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

/// main: c1..c4, each adding its own file (c2's message has a body).
pub(super) fn linear(name: &str) -> (PathBuf, [Oid; 4]) {
    let dir = scratch_repo(name);
    let c1 = commit(&dir, "1.txt", "1\n", "c1");
    let c2 = commit(&dir, "2.txt", "2\n", "c2\n\nThe second commit.");
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    let c4 = commit(&dir, "4.txt", "4\n", "c4");
    (dir, [c1, c2, c3, c4])
}

fn refused(r: crate::error::Result<RebasePlan>) -> String {
    match r {
        Err(Error::Unsupported(text)) => text,
        Err(e) => panic!("expected a refusal, got {e:?}"),
        Ok(p) => panic!("expected a refusal, got {p:?}"),
    }
}

fn oids(p: &RebasePlan) -> Vec<Oid> {
    p.steps.iter().filter_map(TodoStep::oid).collect()
}

fn items(p: &RebasePlan) -> Vec<&TodoItem> {
    p.steps.iter().filter_map(TodoStep::commit).collect()
}

#[test]
fn from_a_commit() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-plan-from");
    let p = plan(&dir, &RebaseBase::FromCommit(c2)).unwrap();
    assert_eq!(oids(&p), [c2, c3, c4], "oldest first, the clicked commit included");
    assert_eq!((p.head, p.branch.as_str(), p.upstream), (c4, "main", Upstream::Commit(c1)));
    assert_eq!(p.onto_label, format!("{} c1", &c1.to_string()[..7]));
    assert!(items(&p).iter().all(|i| i.action == RebaseAction::Pick && i.author == "Test" && i.new_message.is_none()));
    assert_eq!(p.steps.len(), 3, "a range without merges has commit steps only");
    assert_eq!(items(&p)[0].summary, "c2");
    assert_eq!(items(&p)[0].message, "c2\n\nThe second commit.\n");

    let p = plan(&dir, &RebaseBase::FromCommit(c1)).unwrap();
    assert_eq!(oids(&p), [c1, c2, c3, c4]);
    assert_eq!((p.upstream, p.onto_label.as_str()), (Upstream::Root, ""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn onto_a_branch() {
    let dir = scratch_repo("rebase-plan-onto");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "base"]);
    let tip = commit(&dir, "b.txt", "b\n", "on base");
    git(&dir, &["checkout", "-q", "-b", "feature", "HEAD~1"]);
    let f1 = commit(&dir, "f.txt", "1\n", "f1");
    let f2 = commit(&dir, "f.txt", "2\n", "f2");
    let p = plan(&dir, &RebaseBase::OntoBranch("base".into())).unwrap();
    assert_eq!(oids(&p), [f1, f2]);
    assert_eq!((p.upstream, p.onto_label.as_str(), p.branch.as_str()), (Upstream::Commit(tip), "base", "feature"));
    let e = plan(&dir, &RebaseBase::OntoBranch("missing".into())).unwrap_err();
    assert!(e.is_not_found(), "{e:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals() {
    let (dir, [c1, c2, _, c4]) = linear("rebase-plan-refused");
    // A commit on another branch.
    git(&dir, &["checkout", "-q", "-b", "side", &c2.to_string()]);
    let side = commit(&dir, "s.txt", "s\n", "side");
    git(&dir, &["checkout", "-q", "main"]);
    let text = refused(plan(&dir, &RebaseBase::FromCommit(side)));
    assert_eq!(text, format!("{} is not on the current branch", &side.to_string()[..7]));

    // An empty range: main is already on a branch that contains it.
    git(&dir, &["branch", "same", "main"]);
    assert_eq!(refused(plan(&dir, &RebaseBase::OntoBranch("same".into()))), "main has no commits that aren't already on same");

    // A merge in the range is planned, with the steps that recreate it.
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let merge = head(&dir);
    commit(&dir, "5.txt", "5\n", "c5");
    let p = plan(&dir, &RebaseBase::FromCommit(c4)).unwrap();
    assert!(p.steps.iter().any(|s| s.merge().is_some_and(|m| m.item.oid == merge)), "{:?}", p.steps);
    assert!(p.steps.iter().any(|s| matches!(s, TodoStep::Reset(_))));
    // The merge itself as the clicked commit: it and its side line.
    let p = plan(&dir, &RebaseBase::FromCommit(merge)).unwrap();
    assert_eq!(p.upstream, Upstream::Commit(c4), "the upstream is the merge's first parent");
    assert_eq!(oids(&p), [side, merge, head(&dir)]);

    // Detached HEAD.
    git(&dir, &["checkout", "-q", &c4.to_string()]);
    assert_eq!(refused(plan(&dir, &RebaseBase::FromCommit(c1))), DETACHED);
    git(&dir, &["checkout", "-q", "main"]);

    // A rebase already in progress.
    git(&dir, &["checkout", "-q", "-b", "conflicting", &c1.to_string()]);
    commit(&dir, "5.txt", "other\n", "conflicting c5");
    let (ok, _) = run(&dir, &args(&["rebase", "main"]));
    assert!(!ok, "the rebase should stop on a conflict");
    let text = refused(plan(&dir, &RebaseBase::FromCommit(c1)));
    assert_eq!(text, "Finish or abort the rebase in progress before starting an interactive rebase.");
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn onto_a_commit() {
    let (dir, [c1, c2, ..]) = linear("rebase-plan-onto-commit");
    git(&dir, &["checkout", "-q", "-b", "other", &c1.to_string()]);
    let o = commit(&dir, "o.txt", "o\n", "on other");
    git(&dir, &["checkout", "-q", "main"]);
    let p = plan(&dir, &RebaseBase::OntoCommit(o)).unwrap();
    assert_eq!(p.upstream, Upstream::Commit(o));
    assert_eq!(p.onto_label, format!("{} on other", &o.to_string()[..7]));
    assert_eq!(oids(&p).len(), 3, "c2..c4, the commits other lacks");
    assert_eq!(oids(&p)[0], c2);

    let missing = Oid::from_bytes(&[7; 20]).unwrap();
    assert!(plan(&dir, &RebaseBase::OntoCommit(missing)).unwrap_err().is_not_found());
    let tip = head(&dir);
    let text = refused(plan(&dir, &RebaseBase::OntoCommit(tip)));
    assert_eq!(text, format!("main has no commits that aren't already on {} c4", &tip.to_string()[..7]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plan_rows_read_commit_text_in_its_encoding() {
    let dir = scratch_repo("rebase-plan-latin1");
    let c1 = commit(&dir, "1.txt", "1\n", "c1");
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.find_commit(c1).unwrap().tree_id();
    let l1 = raw_commit(&repo, tree, Some(c1), LATIN1_WHO, Some("ISO-8859-1"), b"Caf\xe9 one\n");
    let l2 = raw_commit(&repo, tree, Some(l1), LATIN1_WHO, Some("ISO-8859-1"), b"Cr\xe8me two\n\nbody \xe9\n");
    repo.reference("refs/heads/main", l2, true, "test").unwrap();
    git(&dir, &["reset", "-q", "--hard"]);
    let label = format!("{} Café one", &l1.to_string()[..7]);

    let p = plan(&dir, &RebaseBase::FromCommit(l2)).unwrap();
    assert_eq!(p.onto_label, label);
    let picks = items(&p);
    assert_eq!(picks.len(), 1);
    assert_eq!((picks[0].summary.as_str(), picks[0].message.as_str()), ("Crème two", "Crème two\n\nbody é\n"));
    assert_eq!(picks[0].author, "René Müller");

    git(&dir, &["checkout", "-q", "-b", "side", &c1.to_string()]);
    commit(&dir, "s.txt", "s\n", "side");
    assert_eq!(plan(&dir, &RebaseBase::OntoCommit(l1)).unwrap().onto_label, label);
    let _ = std::fs::remove_dir_all(&dir);
}
