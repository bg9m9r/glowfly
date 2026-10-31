//! Checking out a pull request's source branch (and local branches): the name comes from the
//! code host, so a name like a path must never discard changes and one like an option is refused.

use super::branch_tests::with_origin;
use super::tests::{accept_stash, drive, git};
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::branches::{checkout_local, pr_checkout};
use glowfly_core::gitcli::{args, run};
use std::path::Path;

fn head(dir: &Path) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", "--abbrev-ref", "HEAD"]));
    assert!(ok, "{out}");
    out.trim().to_owned()
}

/// A working file's text with git's line endings (core.autocrlf on Windows) as "\n".
fn text(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap().replace("\r\n", "\n")
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A repository whose tracked file `src` has uncommitted changes, with a branch `src` that
/// exists only on origin.
fn path_named_branch(name: &str) -> (crate::state::AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    std::fs::write(work.join("src"), "committed\n").unwrap();
    git(&work, &["add", "src"]);
    git(&work, &["commit", "-q", "-m", "src"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["branch", "src"]);
    git(&work, &["push", "-q", "origin", "src"]);
    git(&work, &["branch", "-q", "-D", "src"]);
    std::fs::write(work.join("src"), "my unsaved work\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

#[test]
fn a_branch_named_like_a_path_keeps_changes() {
    let (mut st, work, bare) = path_named_branch("pr-checkout-path");
    let cmd = pr_checkout(st.snap.as_ref().unwrap(), "origin", "src").unwrap();
    assert_eq!(cmd, args(&["checkout", "--track", "origin/src", "--"]));
    drive(&mut st, Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds: vec![cmd] }));
    accept_stash(&mut st);
    assert_eq!(head(&work), "src");
    assert_eq!(text(&work, "src"), "my unsaved work\n");

    // Now local: the local branch, still with `--`.
    git(&work, &["switch", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let cmd = pr_checkout(st.snap.as_ref().unwrap(), "origin", "src").unwrap();
    assert_eq!(cmd, checkout_local("src"));
    drive(&mut st, Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds: vec![cmd] }));
    accept_stash(&mut st);
    assert_eq!(head(&work), "src");
    assert_eq!(text(&work, "src"), "my unsaved work\n");
    cleanup(&[&work, &bare]);
}

#[test]
fn option_like_and_unknown_names_are_refused() {
    let (st, work, bare) = with_origin("pr-checkout-refuse");
    let snap = st.snap.as_ref().unwrap();
    assert!(pr_checkout(snap, "origin", "-f").is_err());
    assert!(pr_checkout(snap, "origin", "--orphan=x").is_err());
    assert!(pr_checkout(snap, "origin", "").is_err());
    let why = pr_checkout(snap, "origin", "from-a-fork").unwrap_err();
    assert!(why.contains("fork"), "{why}");
    assert_eq!(pr_checkout(snap, "origin", "feature").unwrap(), checkout_local("feature"));
    cleanup(&[&work, &bare]);
}

#[test]
fn local_checkout_names_no_paths() {
    assert_eq!(checkout_local("src"), args(&["checkout", "src", "--"]));
}
