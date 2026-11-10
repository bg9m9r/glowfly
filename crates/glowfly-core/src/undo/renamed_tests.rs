//! Branch and remote renames: undo renames back (the branch's upstream, config and reflog, or the
//! remote's section, refs and tracking settings move back with it), redo renames again, and the
//! names are compared exactly.

use super::remote_config::read;
use super::tests::{cmds, commit, record, refused, run_plan};
use super::*;
use crate::testutil::*;
use git2::{BranchType, Repository};
use std::path::Path;

fn tip(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().find_reference(&format!("refs/heads/{name}")).ok().and_then(|r| r.target())
}

fn config(dir: &Path, key: &str) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let cfg = repo.config().unwrap().snapshot().unwrap();
    cfg.get_string(key).ok()
}

fn reflog(dir: &Path, name: &str) -> Vec<String> {
    let repo = Repository::open(dir).unwrap();
    let log = repo.reflog(&format!("refs/heads/{name}")).unwrap();
    log.iter().map(|e| e.message().unwrap_or_default().to_owned()).collect()
}

fn branch_names(dir: &Path) -> Vec<String> {
    let repo = Repository::open(dir).unwrap();
    let mut names: Vec<String> =
        repo.branches(Some(BranchType::Local)).unwrap().map(|b| b.unwrap().0.name().unwrap().unwrap().to_owned()).collect();
    names.sort();
    names
}

fn head_branch(dir: &Path) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let h = repo.head().ok()?;
    h.is_branch().then(|| h.shorthand().unwrap().to_owned())
}

#[test]
fn branch_rename_round_trip() {
    let dir = scratch_repo("undo-rename-branch");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["branch", "x"]);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/x", &c1.to_string()]);
    git(&dir, &["branch", "--set-upstream-to", "origin/x", "x"]);
    git(&dir, &["config", "branch.x.description", "The x work"]);
    let log = reflog(&dir, "x");
    assert!(!log.is_empty());

    let action = record(&dir, &["branch", "-m", "--", "x", "y"]).expect("recorded");
    assert_eq!(action, Action::RenameBranch { old: "x".into(), new: "y".into(), remote: None });
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["branch", "-m", "--", "y", "x"]]));
    run_plan(&dir, undo);
    assert_eq!((tip(&dir, "x"), tip(&dir, "y")), (Some(c1), None));
    assert_eq!(config(&dir, "branch.x.remote").as_deref(), Some("origin"));
    assert_eq!(config(&dir, "branch.x.merge").as_deref(), Some("refs/heads/x"));
    assert_eq!(config(&dir, "branch.x.description").as_deref(), Some("The x work"));
    assert_eq!(config(&dir, "branch.y.remote"), None);
    let now = reflog(&dir, "x");
    assert!(now.ends_with(&log), "the original entries are still there: {now:?}");

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["branch", "-m", "--", "x", "y"]]));
    run_plan(&dir, redo);
    assert_eq!((tip(&dir, "x"), tip(&dir, "y")), (None, Some(c1)));
    assert_eq!(config(&dir, "branch.y.description").as_deref(), Some("The x work"));

    // `-M` writes over a branch, so a rename onto an existing name isn't recorded.
    git(&dir, &["branch", "z"]);
    let kind = UndoKind::RenameBranch { old: "y".into(), new: "z".into(), remote: None };
    let b = before(&dir, &kind).unwrap();
    git(&dir, &["branch", "-M", "y", "z"]);
    assert_eq!(after(&dir, kind, b).unwrap(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renaming_the_checked_out_branch_back_keeps_head_on_it() {
    let dir = scratch_repo("undo-rename-head");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let action = record(&dir, &["branch", "-m", "--", "main", "trunk"]).expect("recorded");
    assert_eq!(head_branch(&dir).as_deref(), Some("trunk"));
    run_plan(&dir, plan(&dir, &action, false).unwrap());
    assert_eq!((head_branch(&dir).as_deref(), tip(&dir, "main")), (Some("main"), Some(c1)));
    run_plan(&dir, plan(&dir, &action, true).unwrap());
    assert_eq!((head_branch(&dir).as_deref(), tip(&dir, "trunk")), (Some("trunk"), Some(c1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn branch_rename_refusals() {
    let dir = scratch_repo("undo-rename-refused");
    commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["branch", "x"]);
    let action = record(&dir, &["branch", "-m", "--", "x", "y"]).expect("recorded");

    git(&dir, &["branch", "x"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("A branch named x exists again")), "{r:?}");
    git(&dir, &["branch", "-D", "x"]);
    git(&dir, &["branch", "-D", "y"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("Branch y no longer exists")), "{r:?}");

    // A rename that only changes case: the names are told apart by an exact listing. git refuses
    // it for a loose ref on a file system that ignores case, so the refs are packed first.
    git(&dir, &["branch", "Foo"]);
    git(&dir, &["pack-refs", "--all"]);
    let action = record(&dir, &["branch", "-m", "--", "Foo", "foo"]).expect("recorded");
    assert_eq!(action, Action::RenameBranch { old: "Foo".into(), new: "foo".into(), remote: None });
    git(&dir, &["pack-refs", "--all"]);
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["branch", "-m", "--", "foo", "Foo"]]));
    run_plan(&dir, undo);
    assert_eq!(branch_names(&dir), ["Foo", "main"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A remote whose fetch refspecs come last, where `git remote rename` writes them: 2 URLs, 2
/// push URLs, tagopt, the default refspec and a custom one; main tracks it.
fn rich_remote(dir: &Path, c1: Oid) {
    for (k, v) in [
        ("url", "https://example.invalid/a.git"),
        ("url", "https://example.invalid/b.git"),
        ("pushurl", "https://example.invalid/push-a.git"),
        ("pushurl", "https://example.invalid/push-b.git"),
        ("tagopt", "--no-tags"),
        ("fetch", "+refs/heads/*:refs/remotes/origin/*"),
        ("fetch", "+refs/heads/main:refs/remotes/origin/main-only"),
    ] {
        git(dir, &["config", "--add", &format!("remote.origin.{k}"), v]);
    }
    git(dir, &["config", "branch.main.remote", "origin"]);
    git(dir, &["config", "branch.main.merge", "refs/heads/main"]);
    git(dir, &["update-ref", "refs/remotes/origin/main", &c1.to_string()]);
}

#[test]
fn remote_rename_round_trip_with_several_urls_and_a_custom_refspec() {
    let dir = scratch_repo("undo-rename-remote");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    rich_remote(&dir, c1);
    let repo = Repository::open(&dir).unwrap();
    let snapshot = read(&repo, "origin").unwrap().expect("the section");
    assert_eq!(snapshot.entries.len(), 7);

    let action = record(&dir, &["remote", "rename", "--", "origin", "up"]).expect("recorded");
    assert_eq!(action, Action::RenameRemote { old: "origin".into(), new: "up".into(), urls: None });
    assert_eq!(config(&dir, "branch.main.remote").as_deref(), Some("up"));
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["remote", "rename", "--", "up", "origin"]]));
    run_plan(&dir, undo);
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(read(&repo, "origin").unwrap(), Some(snapshot), "the same values, in order, repeats included");
    assert_eq!(read(&repo, "up").unwrap(), None);
    assert!(repo.find_reference("refs/remotes/origin/main").is_ok());
    assert!(repo.find_reference("refs/remotes/up/main").is_err());
    assert_eq!(config(&dir, "branch.main.remote").as_deref(), Some("origin"));

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["remote", "rename", "--", "origin", "up"]]));
    run_plan(&dir, redo);
    assert!(Repository::open(&dir).unwrap().find_reference("refs/remotes/up/main").is_ok());

    // A remote named like the old one again blocks the undo; a gone one too.
    git(&dir, &["remote", "add", "origin", "https://example.invalid/c.git"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("A remote named origin exists again")), "{r:?}");
    git(&dir, &["remote", "remove", "origin"]);
    git(&dir, &["remote", "remove", "up"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("Remote up no longer exists")), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remote_rename_with_url_edit_puts_the_urls_back() {
    let dir = scratch_repo("undo-rename-remote-urls");
    commit(&dir, "a.txt", "one\n", "c1");
    let (old_url, old_push) = ("https://example.invalid/old.git", "https://example.invalid/old-push.git");
    let (new_url, new_push) = ("https://example.invalid/new.git", "https://example.invalid/new-push.git");
    git(&dir, &["remote", "add", "origin", old_url]);
    git(&dir, &["remote", "set-url", "--push", "--", "origin", old_push]);

    let kind = UndoKind::RenameRemote { old: "origin".into(), new: "up".into() };
    let b = before(&dir, &kind).unwrap();
    git(&dir, &["remote", "rename", "--", "origin", "up"]);
    git(&dir, &["remote", "set-url", "--", "up", new_url]);
    git(&dir, &["remote", "set-url", "--push", "--", "up", new_push]);
    let action = after(&dir, kind, b).unwrap().expect("recorded");
    let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
    let change = UrlChange {
        before: vec![pair("url", old_url), pair("pushurl", old_push)],
        after: vec![pair("url", new_url), pair("pushurl", new_push)],
    };
    assert_eq!(action, Action::RenameRemote { old: "origin".into(), new: "up".into(), urls: Some(change) });

    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(
        undo,
        cmds(&[
            &["remote", "rename", "--", "up", "origin"],
            &["config", "--unset-all", "remote.origin.url"],
            &["config", "--add", "remote.origin.url", old_url],
            &["config", "--unset-all", "remote.origin.pushurl"],
            &["config", "--add", "remote.origin.pushurl", old_push],
        ])
    );
    run_plan(&dir, undo);
    let urls = |name: &str| (config(&dir, &format!("remote.{name}.url")), config(&dir, &format!("remote.{name}.pushurl")));
    assert_eq!(urls("origin"), (Some(old_url.to_owned()), Some(old_push.to_owned())));
    run_plan(&dir, plan(&dir, &action, true).unwrap());
    assert_eq!(urls("up"), (Some(new_url.to_owned()), Some(new_push.to_owned())));

    // A URL changed again after the edit: putting the old ones back would lose it.
    git(&dir, &["remote", "set-url", "--", "up", "https://example.invalid/later.git"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("Remote up has changed since it was edited")), "{r:?}");
    // A rename alone doesn't look at the URLs.
    let plain = Action::RenameRemote { old: "origin".into(), new: "up".into(), urls: None };
    assert_eq!(plan(&dir, &plain, false).unwrap(), cmds(&[&["remote", "rename", "--", "up", "origin"]]));
    let _ = std::fs::remove_dir_all(&dir);
}
