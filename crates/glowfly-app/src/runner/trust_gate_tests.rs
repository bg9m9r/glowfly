//! The per-command trust gate: a command after a fetch in the same job is compared with what
//! that fetch brought, and a job stopped part way says why.

use super::*;
use crate::effect::{Sealed, TrustCheck};
use crate::msg::{JobMsg, Msg, TrustMsg};
use glowfly_core::testkit::git;
use std::path::{Path, PathBuf};

/// A working repository whose `core.hooksPath` is its tracked `.githooks` (a post-merge hook
/// there), pushed to a bare origin beside it; and a second clone to push changes from.
fn following(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-gate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (work, bare, other) = (base.join("work"), base.join("origin.git"), base.join("other"));
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&work, &["config", k, v]);
    }
    std::fs::create_dir_all(work.join(".githooks")).unwrap();
    std::fs::write(work.join(".githooks/post-merge"), "#!/bin/sh\nexit 0\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "hooks"]);
    git(&work, &["remote", "add", "origin", &bare.to_string_lossy()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    git(&base, &["clone", "-q", &bare.to_string_lossy(), "other"]);
    for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false"), ("core.hooksPath", ".git/hooks")] {
        git(&other, &["config", k, v]);
    }
    (work, bare, other)
}

fn push_change(other: &Path, file: &str, text: &str) {
    std::fs::write(other.join(file), text).unwrap();
    git(other, &["add", "."]);
    git(other, &["commit", "-q", "-m", file]);
    git(other, &["push", "-q", "origin", "main"]);
}

fn job(dir: &Path, cmds: &[&[&str]]) -> Effect {
    Effect::RunGit {
        cwd: dir.into(),
        cmds: cmds.iter().map(|c| glowfly_core::gitcli::args(c)).collect(),
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
    }
}

fn trusted(dir: &Path) -> TrustCheck {
    let print = glowfly_core::trust::scan_at(dir).unwrap().print();
    TrustCheck {
        workdir: dir.into(),
        real: dir.into(),
        saved: Some(glowfly_core::settings::SavedTrust::trusted(Some(print))),
        sealed: Sealed::Job { label: "Update".into() },
        accept_unchecked: false,
    }
}

fn head(dir: &Path) -> Option<git2::Oid> {
    git2::Repository::open(dir).unwrap().head().unwrap().target()
}

#[test]
fn a_merge_after_a_fetch_in_the_same_job_is_compared_with_what_the_fetch_brought() {
    let (work, _bare, other) = following("toctou");
    let before = head(&work);
    // When the job was sent, origin/main still had the trusted hook; its fetch brings another.
    push_change(&other, ".githooks/post-merge", "#!/bin/sh\necho changed\n");
    let fx = job(&work, &[&["fetch", "-q"], &["merge", "--ff-only", "origin/main"]]);
    match crate::runner::execute_checked(trusted(&work), fx) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => {
            assert_eq!(s.incoming.as_ref().map(|i| i.hooks.clone()), Some(vec!["post-merge".to_owned()]));
            assert!(matches!(s.effect, Effect::RunGit { ref cmds, .. } if cmds.len() == 2), "the whole job waits again");
        }
        other => panic!("held before the merge: {}", other.is_some()),
    }
    assert_eq!(head(&work), before, "nothing merged");
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn a_job_stopped_after_it_changed_something_fails_saying_why() {
    let (work, _bare, other) = following("partway");
    push_change(&other, ".githooks/post-merge", "#!/bin/sh\necho changed\n");
    let fx = job(&work, &[&["fetch", "-q"], &["tag", "before"], &["merge", "--ff-only", "origin/main"]]);
    match crate::runner::execute_checked(trusted(&work), fx).map(|m| match m {
        Msg::Scoped { msg, .. } => *msg,
        m => m,
    }) {
        Some(Msg::Job(JobMsg::JobDone { ok: false, output, .. })) => assert!(output.contains("stopped the job"), "{output}"),
        other => panic!("failed part way: {}", other.is_some()),
    }
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn a_pull_job_merges_only_what_its_one_fetch_checked() {
    let (work, _bare, other) = following("pull");
    push_change(&other, "b.txt", "b\n");
    let fx = job(&work, &[&["pull", "--ff-only"]]);
    match crate::runner::execute_checked(trusted(&work), fx).map(|m| match m {
        Msg::Scoped { msg, .. } => *msg,
        m => m,
    }) {
        Some(Msg::Job(JobMsg::JobDone { ok: true, output, .. })) => {
            assert_eq!(output.matches("$ git fetch").count(), 1, "{output}");
            assert!(output.contains("$ git merge --ff-only"), "{output}");
        }
        other => panic!("it runs: {}", other.is_some()),
    }
    assert!(work.join("b.txt").exists());
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn an_undo_compares_every_commit_its_action_names() {
    let a = "1".repeat(40);
    let b = "a".repeat(40);
    let text = format!("Checkout {{ from: Detached({a}), to: Branch(\"main\"), x: {b}, zero: {} }}", "0".repeat(40));
    let found = commits_in(&text);
    assert_eq!(found, [git2::Oid::from_str(&a).unwrap(), git2::Oid::from_str(&b).unwrap()], "the zero id names nothing");
}
