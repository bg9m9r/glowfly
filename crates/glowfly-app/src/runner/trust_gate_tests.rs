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
        accepted: None,
        lang: Default::default(),
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

fn done(msg: Option<Msg>) -> Option<(bool, String)> {
    match msg? {
        Msg::Scoped { msg, .. } => done(Some(*msg)),
        Msg::Job(JobMsg::JobDone { ok, output, .. }) => Some((ok, output)),
        _ => None,
    }
}

#[test]
fn a_command_after_one_that_changed_the_settings_is_held() {
    let (work, _bare, _other) = following("rescan");
    let fx = job(&work, &[&["config", "core.fsmonitor", "evil"], &["status"]]);
    let (ok, output) = done(crate::runner::execute_checked(trusted(&work), fx)).expect("the job ends");
    assert!(!ok && output.contains("changed the repository's settings or hooks"), "{output}");
    assert!(!output.contains("$ git status"), "status never ran: {output}");
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn git_am_of_a_patch_that_writes_a_hook_is_held_and_trust_and_run_takes_only_that() {
    let (work, _bare, other) = following("am");
    std::fs::write(other.join(".githooks/post-applypatch"), "#!/bin/sh\necho ran > applied.txt\n").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-q", "-m", "hook"]);
    let out = work.parent().unwrap().join("patches");
    git(&other, &["format-patch", "-q", "-1", "-o", &out.to_string_lossy()]);
    let patch = std::fs::read_dir(&out).unwrap().flatten().next().unwrap().path();
    let cmds: &[&[&str]] = &[&["am", &patch.to_string_lossy()]];
    let digest = glowfly_core::trust::content_digest(&std::fs::read(&patch).unwrap());
    let reason = glowfly_core::trust::incoming::Unchecked::Patched { path: ".githooks/post-applypatch".into(), digest };
    match crate::runner::execute_checked(trusted(&work), job(&work, cmds)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => {
            assert_eq!(s.unchecked, Some(crate::effect::Accepted { at: Some(0), reason: reason.clone() }));
        }
        other => panic!("held: {}", other.is_some()),
    }
    assert!(!work.join(".githooks/post-applypatch").exists(), "nothing applied");
    // Accepted for another command of the job, it is still held.
    let elsewhere = TrustCheck { accepted: Some(crate::effect::Accepted { at: Some(1), reason: reason.clone() }), ..trusted(&work) };
    assert!(matches!(crate::runner::execute_checked(elsewhere, job(&work, cmds)), Some(Msg::Trust(TrustMsg::Stale(_)))));
    // Accepted for this one, it runs.
    let accepted = TrustCheck { accepted: Some(crate::effect::Accepted { at: Some(0), reason }), ..trusted(&work) };
    let (ok, output) = done(crate::runner::execute_checked(accepted, job(&work, cmds))).expect("it runs");
    assert!(ok, "{output}");
    assert!(work.join(".githooks/post-applypatch").exists());
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn an_acceptance_covers_its_one_item_and_every_other_is_still_compared() {
    let (work, _bare, other) = following("accept-one");
    let out = work.parent().unwrap().join("patches");
    for hook in ["post-applypatch", "pre-commit"] {
        std::fs::write(
            other.join(format!(".githooks/{hook}")),
            "#!/bin/sh
exit 0
",
        )
        .unwrap();
        git(&other, &["add", "."]);
        git(&other, &["commit", "-q", "-m", hook]);
    }
    git(&other, &["format-patch", "-q", "-2", "-o", &out.to_string_lossy()]);
    let mut patches: Vec<PathBuf> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.path()).collect();
    patches.sort();
    let (first, second) = (patches[0].to_string_lossy().into_owned(), patches[1].to_string_lossy().into_owned());
    let cmds: &[&[&str]] = &[&["am", &first, &second]];
    let first_reason = match crate::runner::execute_checked(trusted(&work), job(&work, cmds)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => s.unchecked.expect("unchecked").reason,
        other => panic!("held: {}", other.is_some()),
    };
    // Accepting the first patch leaves the second one held: it writes another hook.
    let accepted = TrustCheck { accepted: Some(crate::effect::Accepted { at: Some(0), reason: first_reason.clone() }), ..trusted(&work) };
    match crate::runner::execute_checked(accepted, job(&work, cmds)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => assert_ne!(s.unchecked.map(|a| a.reason), Some(first_reason)),
        other => panic!("still held: {}", other.is_some()),
    }
    assert!(!work.join(".githooks/pre-commit").exists(), "nothing applied");
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn a_command_the_gate_doesn_t_read_is_held_where_hooks_follow_the_working_tree() {
    let (work, _bare, _other) = following("unmodeled");
    match crate::runner::execute_checked(trusted(&work), job(&work, &[&["bisect", "start"]])) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => {
            assert!(matches!(s.unchecked.map(|a| a.reason), Some(glowfly_core::trust::incoming::Unchecked::Unmodeled(_))));
        }
        other => panic!("held: {}", other.is_some()),
    }
    assert!(!work.join(".git/BISECT_START").exists(), "bisect never ran");
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}
