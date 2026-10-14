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
    p.items.iter().map(|i| i.oid).collect()
}

#[test]
fn from_a_commit() {
    let (dir, [c1, c2, c3, c4]) = linear("rebase-plan-from");
    let p = plan(&dir, &RebaseBase::FromCommit(c2)).unwrap();
    assert_eq!(oids(&p), [c2, c3, c4], "oldest first, the clicked commit included");
    assert_eq!((p.head, p.branch.as_str(), p.upstream), (c4, "main", Upstream::Commit(c1)));
    assert_eq!(p.onto_label, format!("{} c1", &c1.to_string()[..7]));
    assert!(p.items.iter().all(|i| i.action == RebaseAction::Pick && i.author == "Test" && i.new_message.is_none()));
    assert_eq!(p.items[0].summary, "c2");
    assert_eq!(p.items[0].message, "c2\n\nThe second commit.\n");

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

    // A merge in the range, and a merge as the clicked commit.
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let merge = head(&dir);
    commit(&dir, "5.txt", "5\n", "c5");
    let text = refused(plan(&dir, &RebaseBase::FromCommit(c4)));
    assert_eq!(text, format!("The range includes the merge commit {}; rebasing merges isn't supported yet", &merge.to_string()[..7]));
    assert!(refused(plan(&dir, &RebaseBase::FromCommit(merge))).contains("merge commit"));

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
