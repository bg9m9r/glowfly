//! A removed remote: undo writes its whole section back (every key, in order, repeats and a
//! value-less key included) with the branch settings that named it, a fetch then brings its
//! remote-tracking branches back, and redo removes it again.

use super::remote_config::read;
use super::tests::{cmds, commit, record, refused, run_plan};
use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn config(dir: &Path, key: &str) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let cfg = repo.config().unwrap().snapshot().unwrap();
    cfg.get_string(key).ok()
}

fn has_ref(dir: &Path, name: &str) -> bool {
    Repository::open(dir).unwrap().find_reference(name).is_ok()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn remove_round_trip_keeps_every_key() {
    let dir = scratch_repo("undo-remove-remote");
    commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["branch", "dev"]);
    let keys = [
        ("url", "https://example.invalid/a.git"),
        ("url", "https://example.invalid/b.git"),
        ("pushurl", "https://example.invalid/push-a.git"),
        ("pushurl", "https://example.invalid/push-b.git"),
        ("fetch", "+refs/heads/*:refs/remotes/team.alpha/*"),
        ("fetch", "+refs/heads/main:refs/remotes/team.alpha/main-only"),
        ("tagopt", "--no-tags"),
        ("prune", "true"),
        ("promisor", "true"),
    ];
    for (k, v) in keys {
        git(&dir, &["config", "--add", &format!("remote.team.alpha.{k}"), v]);
    }
    add_bare_key(&dir, r#"remote "team.alpha""#, "mirror");
    git(&dir, &["config", "branch.main.remote", "team.alpha"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    git(&dir, &["config", "branch.dev.pushRemote", "team.alpha"]);
    git(&dir, &["config", "remote.pushDefault", "team.alpha"]);
    // Another remote with a name that starts the same is no part of it.
    git(&dir, &["remote", "add", "team", "https://example.invalid/team.git"]);

    let action = record(&dir, &["remote", "remove", "--", "team.alpha"]).expect("recorded");
    let Action::RemoveRemote { name, config: recorded } = &action else { panic!("{action:?}") };
    assert_eq!(name, "team.alpha");
    let mut want = pairs(&keys);
    want.push(("mirror".into(), "true".into()));
    assert_eq!(recorded.entries, want);
    let link = |b: &str, k: &str, v: &str| BranchLink { branch: b.into(), key: k.into(), value: v.into() };
    assert_eq!(
        recorded.branches,
        [link("main", "remote", "team.alpha"), link("main", "merge", "refs/heads/main"), link("dev", "pushremote", "team.alpha")]
    );
    assert!(recorded.push_default);
    assert_eq!(config(&dir, "branch.main.remote"), None, "git unset the branch's upstream");

    run_plan(&dir, plan(&dir, &action, false).unwrap());
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(read(&repo, "team.alpha").unwrap().map(|c| c.entries), Some(want));
    assert_eq!(config(&dir, "branch.main.remote").as_deref(), Some("team.alpha"));
    assert_eq!(config(&dir, "branch.main.merge").as_deref(), Some("refs/heads/main"));
    assert_eq!(config(&dir, "branch.dev.pushremote").as_deref(), Some("team.alpha"));
    assert_eq!(config(&dir, "remote.pushdefault").as_deref(), Some("team.alpha"));
    assert_eq!(config(&dir, "remote.team.url").as_deref(), Some("https://example.invalid/team.git"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fetch_after_undo_brings_tracking_branches_back() {
    let (work, bare) = with_origin("undo-remove-remote-fetch");
    assert!(has_ref(&work, "refs/remotes/origin/main"));
    let action = record(&work, &["remote", "remove", "--", "origin"]).expect("recorded");
    assert!(!has_ref(&work, "refs/remotes/origin/main"));
    run_plan(&work, plan(&work, &action, false).unwrap());
    assert!(!has_ref(&work, "refs/remotes/origin/main"), "only a fetch brings the remote branches back");
    let (ok, out) = crate::gitcli::run(&work, &crate::remote_ops::fetch_cmd("origin"));
    assert!(ok, "{out}");
    assert!(has_ref(&work, "refs/remotes/origin/main"));
    let repo = Repository::open(&work).unwrap();
    let main = repo.find_branch("main", git2::BranchType::Local).unwrap();
    assert_eq!(main.upstream().unwrap().name().unwrap(), Some("origin/main"));
    for d in [&work, &bare] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn links_are_not_written_over() {
    let dir = scratch_repo("undo-remove-remote-links");
    commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["branch", "dev"]);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/a.git"]);
    git(&dir, &["remote", "add", "other", "https://example.invalid/b.git"]);
    for b in ["main", "dev"] {
        git(&dir, &["config", &format!("branch.{b}.remote"), "origin"]);
        git(&dir, &["config", &format!("branch.{b}.merge"), &format!("refs/heads/{b}")]);
    }
    let action = record(&dir, &["remote", "remove", "--", "origin"]).expect("recorded");
    // main gets a new upstream, dev goes.
    git(&dir, &["config", "branch.main.remote", "other"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    git(&dir, &["branch", "-D", "dev"]);
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(
        undo,
        cmds(&[
            &["config", "--add", "remote.origin.url", "https://example.invalid/a.git"],
            &["config", "--add", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"],
        ])
    );
    run_plan(&dir, undo);
    assert_eq!(config(&dir, "branch.main.remote").as_deref(), Some("other"));
    assert_eq!(config(&dir, "branch.dev.remote"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals_and_redo() {
    let dir = scratch_repo("undo-remove-remote-redo");
    commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/a.git"]);
    let action = record(&dir, &["remote", "rm", "origin"]).expect("recorded");

    git(&dir, &["remote", "add", "origin", "https://example.invalid/other.git"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("A remote named origin exists again")), "{r:?}");
    git(&dir, &["remote", "remove", "origin"]);
    let r = refused(plan(&dir, &action, true).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("Remote origin no longer exists")), "{r:?}");

    run_plan(&dir, plan(&dir, &action, false).unwrap());
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["remote", "remove", "--", "origin"]]));
    run_plan(&dir, redo);
    assert_eq!(config(&dir, "remote.origin.url"), None);

    run_plan(&dir, plan(&dir, &action, false).unwrap());
    git(&dir, &["remote", "set-url", "origin", "https://example.invalid/changed.git"]);
    let r = refused(plan(&dir, &action, true).unwrap());
    assert!(matches!(&r, Refusal::Stale(m) if m.starts_with("Remote origin has changed since it was put back")), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
