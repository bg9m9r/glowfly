//! Creating a branch or tag, and checking out a new branch: undo removes what was made (only
//! while it is as it was made), redo makes it again at the same commit or tag object.

use super::tests::{cmds, commit, head_oid, record, refused, run_plan};
use super::*;
use crate::testutil::*;
use git2::{BranchType, Repository};
use std::path::Path;

fn tip(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().find_branch(name, BranchType::Local).ok().and_then(|b| b.get().target())
}

fn upstream(dir: &Path, name: &str) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let b = repo.find_branch(name, BranchType::Local).ok()?;
    b.upstream().ok().and_then(|u| u.name().ok().flatten().map(str::to_owned))
}

fn tag_target(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().find_reference(&format!("refs/tags/{name}")).ok().and_then(|r| r.target())
}

fn current_branch(dir: &Path) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let h = repo.head().ok()?;
    h.is_branch().then(|| h.shorthand().unwrap().to_owned())
}

#[test]
fn created_branch_undo_and_redo() {
    let dir = scratch_repo("undo-create-branch");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/x", &c1.to_string()]);

    let action = record(&dir, &["branch", "x", "origin/x"]).expect("recorded");
    assert_eq!(action, Action::CreateBranch { name: "x".into(), oid: c1, upstream: Some("origin/x".into()) });
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["branch", "-D", "x"]]));
    run_plan(&dir, undo);
    assert_eq!(tip(&dir, "x"), None);

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["branch", "x", &c1.to_string()], &["branch", "--set-upstream-to", "origin/x", "x"]]));
    run_plan(&dir, redo);
    assert_eq!((tip(&dir, "x"), upstream(&dir, "x").as_deref()), (Some(c1), Some("origin/x")));
    let kind = UndoKind::CreateBranch("x".into());
    let b = before(&dir, &kind).unwrap();
    assert_eq!(after(&dir, kind, b).unwrap(), None, "a branch that already existed wasn't created");

    // Checked out: deleting it would fail, so the entry stays for later.
    git(&dir, &["checkout", "-q", "x"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Unsafe(m) if m.contains("checked out")), "{r:?}");
    assert!(!r.is_stale());
    assert_eq!(tip(&dir, "x"), Some(c1), "the branch is still there");

    git(&dir, &["checkout", "-q", "main"]);
    let c2 = commit(&dir, "a.txt", "two\n", "c2");
    git(&dir, &["branch", "-f", "x", &c2.to_string()]);
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale(), "the branch moved since it was created");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lightweight_and_annotated_tags_come_back_exactly() {
    let dir = scratch_repo("undo-create-tag");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let c2 = commit(&dir, "a.txt", "two\n", "c2");

    let light = record(&dir, &["tag", "v1", &c1.to_string()]).expect("recorded");
    assert_eq!(light, Action::CreateTag { name: "v1".into(), oid: c1 });
    let undo = plan(&dir, &light, false).unwrap();
    assert_eq!(undo, cmds(&[&["tag", "-d", "v1"]]));
    run_plan(&dir, undo);
    assert_eq!(tag_target(&dir, "v1"), None);
    let redo = plan(&dir, &light, true).unwrap();
    assert_eq!(redo, cmds(&[&["update-ref", "refs/tags/v1", &c1.to_string(), ""]]));
    run_plan(&dir, redo);
    assert_eq!(tag_target(&dir, "v1"), Some(c1));

    let annotated = record(&dir, &["tag", "-a", "v2", &c1.to_string(), "-m", "Release two"]).expect("recorded");
    let Action::CreateTag { oid: tag_oid, .. } = annotated else { panic!("{annotated:?}") };
    assert_ne!(tag_oid, c1, "the ref holds the tag object");
    run_plan(&dir, plan(&dir, &annotated, false).unwrap());
    assert_eq!(tag_target(&dir, "v2"), None);
    run_plan(&dir, plan(&dir, &annotated, true).unwrap());
    assert_eq!(tag_target(&dir, "v2"), Some(tag_oid));
    let repo = Repository::open(&dir).unwrap();
    let tag = repo.find_tag(tag_oid).unwrap();
    assert_eq!((tag.target_id(), tag.message().map(str::trim)), (c1, Some("Release two")));

    // A tag of that name made again blocks the redo; one moved since blocks the undo.
    run_plan(&dir, plan(&dir, &light, false).unwrap());
    git(&dir, &["tag", "v1", &c2.to_string()]);
    assert!(refused(plan(&dir, &light, true).unwrap()).is_stale());
    git(&dir, &["tag", "-d", "v1"]);
    run_plan(&dir, plan(&dir, &light, true).unwrap());
    git(&dir, &["tag", "-f", "v1", &c2.to_string()]);
    assert!(refused(plan(&dir, &light, false).unwrap()).is_stale());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkout_of_a_new_branch_can_delete_it_on_undo() {
    let dir = scratch_repo("undo-checkout-new");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let action = record(&dir, &["checkout", "-b", "feat"]).expect("recorded");
    let created = Created { name: "feat".into(), oid: c1, upstream: None };
    let want = Action::Checkout { from: HeadPos::Branch("main".into()), to: HeadPos::Branch("feat".into()), created: Some(created) };
    assert_eq!(action, want);
    assert_eq!(action.describe(), "checkout of new branch feat");

    let delete = Options { delete_created: true };
    let undo = plan_with(&dir, &action, false, delete).unwrap();
    assert_eq!(undo, cmds(&[&["checkout", "main", "--"], &["branch", "-D", "feat"]]));
    run_plan(&dir, undo);
    assert_eq!((current_branch(&dir).as_deref(), tip(&dir, "feat")), (Some("main"), None));

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["checkout", "-b", "feat", &c1.to_string(), "--"]]));
    run_plan(&dir, redo);
    assert_eq!((current_branch(&dir).as_deref(), tip(&dir, "feat")), (Some("feat"), Some(c1)));

    // Without deleting, the branch stays and redo only checks it out.
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["checkout", "main", "--"]]));
    run_plan(&dir, undo);
    assert_eq!(tip(&dir, "feat"), Some(c1));
    assert_eq!(plan(&dir, &action, true).unwrap(), cmds(&[&["checkout", "feat", "--"]]));
    run_plan(&dir, plan(&dir, &action, true).unwrap());

    // A commit on the new branch: deleting it would lose the commit.
    commit(&dir, "a.txt", "two\n", "on feat");
    let moved = refused(plan_with(&dir, &action, false, delete).unwrap());
    assert!(matches!(&moved, Refusal::Unsafe(m) if m.contains("commits made after")), "{moved:?}");
    assert!(record(&dir, &["checkout", "main"]).is_some_and(|a| matches!(a, Action::Checkout { created: None, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkout_track_records_the_upstream() {
    let (work, bare) = with_origin("undo-checkout-track");
    git(&work, &["push", "-q", "origin", "main:x"]);
    let oid = head_oid(&work);
    let action = record(&work, &["checkout", "--track", "origin/x"]).expect("recorded");
    let Action::Checkout { created: Some(c), .. } = &action else { panic!("{action:?}") };
    assert_eq!(c, &Created { name: "x".into(), oid, upstream: Some("origin/x".into()) });

    run_plan(&work, plan_with(&work, &action, false, Options { delete_created: true }).unwrap());
    assert_eq!(tip(&work, "x"), None);
    let redo = plan(&work, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["checkout", "-b", "x", &oid.to_string(), "--"], &["branch", "--set-upstream-to", "origin/x", "x"]]));
    run_plan(&work, redo);
    assert_eq!((current_branch(&work).as_deref(), upstream(&work, "x").as_deref()), (Some("x"), Some("origin/x")));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_branch_moved_with_update_ref_undoes_and_redoes() {
    let dir = scratch_repo("undo-set-branch");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let c2 = commit(&dir, "a.txt", "two\n", "c2");
    git(&dir, &["branch", "x", &c2.to_string()]);
    let cmd = crate::journal::restore::move_branch("x", c1, c2);
    let kind = UndoKind::classify(&cmd).expect("recognised");
    assert_eq!(kind, UndoKind::SetBranch("x".into()));
    let tag = args(&["update-ref", "-m", "r", "refs/tags/v1", &c1.to_string(), &c2.to_string()]);
    assert_eq!(UndoKind::classify(&tag), None, "only branches");
    assert_eq!(UndoKind::classify(&args(&["update-ref", "-m", "r", "refs/heads/x", "HEAD", &c2.to_string()])), None);

    let b = before(&dir, &kind).unwrap();
    let (ok, out) = crate::gitcli::run(&dir, &cmd);
    assert!(ok, "{out}");
    let action = after(&dir, kind, b).unwrap().expect("recorded");
    assert_eq!(action, Action::SetBranch { name: "x".into(), from: c2, to: c1 });
    assert_eq!(action.describe(), format!("move of branch x to {}", &c1.to_string()[..7]));

    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["update-ref", "-m", "Glowfly: undo move of x", "refs/heads/x", &c2.to_string(), &c1.to_string()]]));
    run_plan(&dir, undo);
    assert_eq!(tip(&dir, "x"), Some(c2));
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["update-ref", "-m", "Glowfly: redo move of x", "refs/heads/x", &c1.to_string(), &c2.to_string()]]));
    run_plan(&dir, redo);
    assert_eq!(tip(&dir, "x"), Some(c1));

    // Moved since: stale. Checked out: the entry stays.
    git(&dir, &["branch", "-f", "x", &c2.to_string()]);
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale());
    git(&dir, &["branch", "-f", "x", &c1.to_string()]);
    git(&dir, &["checkout", "-q", "x"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Unsafe(m) if m.contains("checked out")), "{r:?}");
    git(&dir, &["checkout", "-q", "main"]);
    let missing = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    let ghost = Action::SetBranch { name: "x".into(), from: missing, to: c1 };
    assert!(matches!(refused(plan(&dir, &ghost, false).unwrap()), Refusal::Stale(m) if m.contains("gone")));
    git(&dir, &["branch", "-D", "x"]);
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale(), "the branch is gone");
    let _ = std::fs::remove_dir_all(&dir);
}
