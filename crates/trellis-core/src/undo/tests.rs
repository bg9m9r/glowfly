use super::*;
use crate::gitcli::run;
use crate::repo::status_at;
use crate::testutil::*;
use git2::{BranchType, Repository};
use std::path::Path;

/// Runs `git <a>` as an undoable operation and returns what it recorded.
fn record(dir: &Path, a: &[&str]) -> Option<Action> {
    let argv = args(a);
    let kind = UndoKind::classify(&argv).expect("an undoable command");
    let b = before(dir, &kind).unwrap();
    git(dir, a);
    after(dir, kind, b).unwrap()
}

fn run_plan(dir: &Path, plan: Plan) {
    let Plan::Run(cmds) = plan else { panic!("expected commands, got {plan:?}") };
    for c in cmds {
        let (ok, out) = run(dir, &c);
        assert!(ok, "git {c:?} failed: {out}");
    }
}

fn head_oid(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
    head_oid(dir)
}

fn refused(plan: Plan) -> Refusal {
    match plan {
        Plan::Refuse(r) => r,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn cmds(list: &[&[&str]]) -> Plan {
    Plan::Run(list.iter().map(|a| args(a)).collect())
}

#[test]
fn describe_names_the_action() {
    let o = Oid::from_str("1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b").unwrap();
    let checkout = |to| Action::Checkout { from: HeadPos::Branch("main".into()), to };
    assert_eq!(checkout(HeadPos::Branch("feature".into())).describe(), "checkout of feature");
    assert_eq!(checkout(HeadPos::Detached(o)).describe(), "checkout of 1a2b3c4");
    let commit = |message: &str, amend| Action::Commit { branch: None, from: o, to: o, message: message.into(), amend };
    assert_eq!(commit("Fix build on Windows\n\nDetails", false).describe(), "commit 'Fix build on Windows'");
    assert_eq!(commit("Fix", true).describe(), "amend 'Fix'");
    let long = "x".repeat(60);
    assert_eq!(commit(&long, false).describe(), format!("commit '{}…'", "x".repeat(50)));
    assert_eq!(Action::Reset { mode: ResetMode::Mixed, branch: None, from: o, to: o, stash: None }.describe(), "mixed reset to 1a2b3c4");
    assert_eq!(Action::DeleteBranch { name: "feature".into(), oid: o, upstream: None }.describe(), "delete of branch feature");
    assert_eq!(Action::DiscardAll { stash: o, head: o }.describe(), "Discard all changes");
}

#[test]
fn commit_undo_and_redo() {
    let dir = scratch_repo("undo-commit");
    let base = commit(&dir, "a.txt", "one\n", "base");
    write(&dir, "a.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    let action = record(&dir, &["commit", "-q", "-m", "second"]).expect("recorded");
    let second = head_oid(&dir);
    let want = Action::Commit { branch: Some("main".into()), from: base, to: second, message: "second".into(), amend: false };
    assert_eq!(action, want);

    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--soft", &base.to_string()]]));
    run_plan(&dir, undo);
    assert_eq!(head_oid(&dir), base);
    let (_, staged, _) = status_at(&dir, true).unwrap();
    assert_eq!(staged.len(), 1, "the undone commit's changes stay staged");

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["reset", "--soft", &second.to_string()]]));
    run_plan(&dir, redo);
    assert_eq!(head_oid(&dir), second);

    commit(&dir, "b.txt", "b\n", "third");
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn root_commit_is_not_undoable() {
    let dir = scratch_repo("undo-root");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    assert_eq!(record(&dir, &["commit", "-q", "-m", "first"]), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkout_undo_branch_and_detached() {
    let dir = scratch_repo("undo-checkout");
    let base = commit(&dir, "a.txt", "one\n", "base");
    git(&dir, &["branch", "feature"]);
    let to_feature = record(&dir, &["checkout", "feature"]).expect("recorded");
    assert_eq!(to_feature, Action::Checkout { from: HeadPos::Branch("main".into()), to: HeadPos::Branch("feature".into()) });
    assert_eq!(plan(&dir, &to_feature, false).unwrap(), cmds(&[&["checkout", "main", "--"]]));

    let detach = record(&dir, &["checkout", &base.to_string()]).expect("recorded");
    assert_eq!(detach, Action::Checkout { from: HeadPos::Branch("feature".into()), to: HeadPos::Detached(base) });
    assert_eq!(plan(&dir, &detach, false).unwrap(), cmds(&[&["checkout", "feature", "--"]]));

    git(&dir, &["branch", "-D", "feature"]);
    let r = refused(plan(&dir, &detach, false).unwrap());
    assert!(r.is_stale() && r.message().ends_with("undo history cleared"), "{r:?}");

    git(&dir, &["checkout", "-q", "main"]);
    assert_eq!(record(&dir, &["checkout", "main"]), None, "checking out the current branch changes nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hard_reset_undo_needs_clean_tree() {
    let dir = scratch_repo("undo-reset");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let c2 = commit(&dir, "a.txt", "one\ntwo\n", "c2");
    let action = record(&dir, &["reset", "--hard", "HEAD~1"]).expect("recorded");
    assert_eq!(action, Action::Reset { mode: ResetMode::Hard, branch: Some("main".into()), from: c2, to: c1, stash: None });

    write(&dir, "untracked.txt", "new\n");
    assert!(matches!(refused(plan(&dir, &action, false).unwrap()), Refusal::Dirty(m) if m.contains("hard reset")));
    std::fs::remove_file(dir.join("untracked.txt")).unwrap();
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--hard", &c2.to_string()]]));
    run_plan(&dir, undo);
    assert_eq!(head_oid(&dir), c2);

    // Soft undo keeps the index and working tree, so local and staged changes don't block it.
    // (The soft reset itself left c2's change staged.)
    let soft = record(&dir, &["reset", "--soft", "HEAD~1"]).expect("recorded");
    write(&dir, "untracked.txt", "new\n");
    assert_eq!(plan(&dir, &soft, false).unwrap(), cmds(&[&["reset", "--soft", &c2.to_string()]]));

    // A mixed reset rewrites the index, so staged changes block it; unstaged ones don't.
    let mixed = Action::Reset { mode: ResetMode::Mixed, branch: Some("main".into()), from: c2, to: c1, stash: None };
    assert!(matches!(refused(plan(&dir, &mixed, false).unwrap()), Refusal::Dirty(m) if m.contains("staged")));
    run(&dir, &args(&["reset", "-q"]));
    assert_eq!(plan(&dir, &mixed, false).unwrap(), cmds(&[&["reset", "--mixed", &c2.to_string()]]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn branch_delete_restores_tip_and_upstream() {
    let dir = scratch_repo("undo-branch");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["branch", "feature"]);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/feature", &c1.to_string()]);
    git(&dir, &["config", "branch.feature.remote", "origin"]);
    git(&dir, &["config", "branch.feature.merge", "refs/heads/feature"]);

    let action = record(&dir, &["branch", "-D", "feature"]).expect("recorded");
    assert_eq!(action, Action::DeleteBranch { name: "feature".into(), oid: c1, upstream: Some("origin/feature".into()) });
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["branch", "feature", &c1.to_string()], &["branch", "--set-upstream-to", "origin/feature", "feature"]]));
    run_plan(&dir, undo);
    let repo = Repository::open(&dir).unwrap();
    let b = repo.find_branch("feature", BranchType::Local).unwrap();
    assert_eq!(b.get().target(), Some(c1));
    assert_eq!(b.upstream().unwrap().name().unwrap(), Some("origin/feature"));

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["branch", "-D", "feature"]]));
    run_plan(&dir, redo);
    git(&dir, &["branch", "feature"]);
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale(), "a new branch with that name blocks the undo");

    git(&dir, &["branch", "-D", "feature"]);
    git(&dir, &["update-ref", "-d", "refs/remotes/origin/feature"]);
    assert_eq!(plan(&dir, &action, false).unwrap(), cmds(&[&["branch", "feature", &c1.to_string()]]), "no upstream to restore");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_all_round_trip() {
    let dir = scratch_repo("undo-discard");
    write(&dir, "b.txt", "b\n");
    let base = commit(&dir, "a.txt", "a\n", "base");
    write(&dir, "a.txt", "a staged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "b.txt", "b unstaged\n");
    write(&dir, "c.txt", "untracked\n");
    let split = |dir: &Path| {
        let (unstaged, staged, _) = status_at(dir, true).unwrap();
        let names = |v: Vec<crate::repo::FileEntry>| v.into_iter().map(|f| format!("{}{}", f.status, f.path)).collect::<Vec<_>>();
        (names(unstaged), names(staged))
    };
    let dirty = split(&dir);
    assert_eq!(dirty, (vec!["Mb.txt".to_owned(), "?c.txt".to_owned()], vec!["Ma.txt".to_owned()]));

    let action = record(&dir, &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]).expect("recorded");
    let Action::DiscardAll { head, .. } = action.clone() else { panic!("{action:?}") };
    assert_eq!(head, base);
    assert_eq!(split(&dir), (vec![], vec![]));

    // Another stash on top: the discard is now stash@{1}.
    write(&dir, "b.txt", "other\n");
    git(&dir, &["stash", "push", "-q", "-m", "other"]);
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["stash", "pop", "--index", "stash@{1}"]]));
    run_plan(&dir, undo);
    assert_eq!(split(&dir), dirty, "staged, unstaged and untracked come back exactly");
    let mut repo = Repository::open(&dir).unwrap();
    let mut left = 0;
    repo.stash_foreach(|_, _, _| {
        left += 1;
        true
    })
    .unwrap();
    assert_eq!(left, 1);

    assert!(matches!(refused(plan(&dir, &action, false).unwrap()), Refusal::Dirty(_)));
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, Plan::Run(vec![discard_command()]));
    run_plan(&dir, redo);
    let Action::DiscardAll { stash: again, .. } = redone(&dir, action).unwrap() else { panic!() };
    // The same changes in the same second can give the same stash commit, so compare with the
    // stash git just made rather than with the old oid.
    assert_eq!(Some(again), stash_top(&Repository::open(&dir).unwrap()));

    assert_eq!(record(&dir, &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]), None, "nothing to discard");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn busy_repository_refuses() {
    let dir = scratch_repo("undo-busy");
    let base = commit(&dir, "c.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "c.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    let ours = commit(&dir, "c.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");

    let actions = [
        Action::Commit { branch: Some("main".into()), from: base, to: ours, message: "ours".into(), amend: false },
        Action::Checkout { from: HeadPos::Branch("other".into()), to: HeadPos::Branch("main".into()) },
        Action::DiscardAll { stash: base, head: ours },
    ];
    for a in &actions {
        for redo in [false, true] {
            match refused(plan(&dir, a, redo).unwrap()) {
                Refusal::Busy(m) => assert_eq!(m, "Finish or abort the merge in progress first."),
                other => panic!("expected Busy, got {other:?}"),
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
