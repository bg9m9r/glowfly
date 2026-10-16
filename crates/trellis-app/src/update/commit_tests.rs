//! Headless tests for the commit box: stage-all-and-commit, refusals, skipping hooks, pushing
//! after committing, and the commit template.

use super::branch_tests::with_origin;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::CommitMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::commit::pushes_after;
use crate::state::{AppState, RunningJob};
use std::path::Path;
use std::time::Instant;
use trellis_core::gitcli::{args, run};
use trellis_core::repo::FileEntry;

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn head_summary(st: &AppState) -> String {
    st.snap.as_ref().unwrap().commits[0].summary.clone()
}

/// HEAD's full message, as git stores it.
fn head_message(dir: &Path) -> String {
    let (ok, out) = run(dir, &args(&["log", "-1", "--format=%B"]));
    assert!(ok, "{out}");
    out.lines().filter(|l| !l.starts_with("$ ")).collect::<Vec<_>>().join("\n").trim().to_owned()
}

fn rev(dir: &Path, r: &str) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", r]));
    assert!(ok, "{out}");
    out.lines().rfind(|l| !l.starts_with("$ ")).unwrap_or("").trim().to_owned()
}

/// Configures `text` as the repository's commit template.
fn set_template(dir: &Path, text: &str) {
    std::fs::write(dir.join(".git").join("template.txt"), text).unwrap();
    git(dir, &["config", "commit.template", ".git/template.txt"]);
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stage_all_and_commit_end_to_end() {
    let dir = scratch_repo("commit-stage-all");
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.as_ref().unwrap().staged.is_empty());
    st.draft.summary = "Everything".into();
    let fx = update(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }, ..] => assert_eq!(cmds[0], args(&["add", "--all"])),
        other => panic!("expected RunGit, got {other:?}"),
    }
    st.job = None;
    drive(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit));
    assert_eq!(head_summary(&st), "Everything");
    assert!(!st.snap.as_ref().unwrap().is_dirty(), "both files were committed");
    assert!(st.draft.is_empty());
    assert!(st.job.is_none());
    cleanup(&dir);
}

#[test]
fn shortcut_refusals_toast() {
    let dir = scratch_repo("commit-refusals");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let log_before = st.log.len();
    assert!(update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now()).is_empty());
    assert_eq!(last_toast(&st), (false, "Write a summary for the commit.".into()));
    assert!(!st.view.show_log, "a refused shortcut doesn't open the log");
    assert_eq!(st.log.len(), log_before);

    st.draft.summary = "Work".into();
    assert!(update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now()).is_empty(), "nothing staged");
    assert_eq!(last_toast(&st), (false, "Stage files to commit.".into()));
    assert!(!update(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit), Instant::now()).is_empty(), "stage-all has changes to stage");
    st.job = None;

    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("c.txt", '!'));
    assert!(update(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit), Instant::now()).is_empty());
    assert_eq!(last_toast(&st), (false, "Resolve the conflicted files before staging everything.".into()));
    cleanup(&dir);
}

#[test]
fn skip_hooks_bypasses_failing_pre_commit() {
    let dir = scratch_repo("commit-skip-hooks");
    std::fs::write(dir.join(".git").join("hooks").join("pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hook = dir.join(".git").join("hooks").join("pre-commit");
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    // The hook makes the repository ask about trust; trusted, the commit runs it.
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, trellis_core::settings::RepoTrust::Trusted);
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    let head = rev(&dir, "HEAD");
    st.draft.summary = "Hooked".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(rev(&dir, "HEAD"), head, "the pre-commit hook stopped the commit");
    assert_eq!(st.draft.summary, "Hooked", "the draft is kept");
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.starts_with("Commit failed"), "{text}");

    st.draft.skip_hooks = true;
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(head_summary(&st), "Hooked");
    assert!(!st.draft.skip_hooks, "skip hooks resets with the draft");
    cleanup(&dir);
}

#[test]
fn push_after_commit_sets_upstream() {
    let (mut st, work, bare) = with_origin("commit-push-after");
    git(&work, &["checkout", "-q", "-b", "topic"]);
    std::fs::write(work.join("a.txt"), "topic\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.settings.push_after_commit = true;
    st.draft.summary = "Topic work".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(rev(&bare, "refs/heads/topic"), rev(&work, "HEAD"), "the new branch was pushed");
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_info().and_then(|b| b.upstream.clone()).as_deref(), Some("origin/topic"));
    let toasts: Vec<_> = st.toasts.iter().map(|t| t.text.as_str()).collect();
    assert!(toasts.ends_with(&["Commit succeeded", "Push succeeded"]), "{toasts:?}");

    // A branch with an upstream is pushed to it.
    git(&work, &["checkout", "-q", "main"]);
    std::fs::write(work.join("a.txt"), "main\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.draft.summary = "Main work".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    // Both commits can share a timestamp, so the graph may list either first: ask git.
    assert_eq!(head_message(&work), "Main work");
    assert_eq!(st.snap.as_ref().unwrap().head_info().map(|b| b.ahead), Some(0));
    cleanup(&work);
    cleanup(&bare);
}

#[test]
fn no_push_when_commit_fails_or_tab_changed() {
    let dir = scratch_repo("commit-no-push");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.settings.push_after_commit = true;
    let job = |workdir| RunningJob {
        label: "Commit".into(),
        workdir,
        quiet: false,
        clear_commit: true,
        then_push: true,
        then_open: None,
        keeps_undo: false,
    };
    let done = |ok| Msg::Job(JobMsg::JobDone { ok, output: String::new(), undo: None });

    st.job = Some(job(Some(dir.with_file_name("elsewhere"))));
    let fx = update(&mut st, done(true), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "{fx:?}");
    let last = st.log.last().unwrap();
    assert!(last.ok && last.text == "Push after committing was skipped: that repository's tab is no longer active.");

    st.job = Some(job(st.workdir()));
    let fx = update(&mut st, done(false), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "a failed commit doesn't push: {fx:?}");

    // Nothing to push from a detached HEAD.
    git(&dir, &["checkout", "-q", "--detach"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert!(!pushes_after(&st));
    st.draft.summary = "Detached".into();
    update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now());
    assert!(st.job.as_ref().is_some_and(|j| !j.then_push));
    cleanup(&dir);
}

#[test]
fn template_prefills_and_strips_comments() {
    let dir = scratch_repo("commit-template");
    set_template(&dir, "\n\n# Explain why\n");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!((st.draft.summary.as_str(), st.draft.description.as_str()), ("", "# Explain why"));
    assert!(st.draft.template.is_some());

    st.draft.summary = "Templated".into();
    st.draft.description = "Because.\n# Explain why".into();
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(head_message(&dir), "Templated\n\nBecause.", "comment lines are left out");
    assert_eq!(st.draft.description, "# Explain why", "the template is back after the commit");
    assert!(st.draft.is_untouched_template());

    // A draft the user cleared isn't prefilled again.
    st.draft.summary.clear();
    st.draft.description.clear();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.draft.is_empty());
    cleanup(&dir);
}

#[test]
fn template_keeps_issue_mentions() {
    let dir = scratch_repo("commit-template-mention");
    set_template(&dir, "\n\n# Explain why\n");
    git(&dir, &["config", "commit.cleanup", "strip"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.draft.summary = "#42 Fix login".into();
    // What the "Link #123" button sends.
    drive(&mut st, Msg::Commit(CommitMsg::AppendToDescription("#123".into())));
    assert_eq!(st.draft.description, "# Explain why\n\n#123");
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(head_message(&dir), "#42 Fix login\n\n#123", "only the template's comment is left out");
    cleanup(&dir);
}

#[test]
fn untouched_template_refused() {
    let dir = scratch_repo("commit-template-untouched");
    set_template(&dir, "feat: \n\n# body\n");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert_eq!(st.draft.summary, "feat:");
    assert!(update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now()).is_empty());
    assert_eq!(last_toast(&st), (false, "Edit the commit template before committing.".into()));
    cleanup(&dir);
}

#[test]
fn amend_replaces_untouched_template() {
    let dir = scratch_repo("commit-template-amend");
    set_template(&dir, "feat: \n\n# body\n");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.draft.is_untouched_template());
    st.draft.amend = true;
    drive(&mut st, Msg::Commit(CommitMsg::PrefillAmend));
    assert_eq!(st.draft.summary, "first");
    assert!(st.draft.template.is_none(), "an amend of HEAD's message doesn't strip comments");
    cleanup(&dir);
}

#[test]
fn undo_restores_message_over_template() {
    let dir = scratch_repo("commit-template-undo");
    set_template(&dir, "feat: \n\n# body\n");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.draft.summary = "feat: second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert!(st.draft.is_untouched_template(), "prefilled again after the commit");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "first");
    assert_eq!(st.draft.summary, "feat: second line");
    assert!(st.draft.template.is_none());
    cleanup(&dir);
}

#[test]
fn set_push_after_commit_saves() {
    let mut st = state();
    let fx = update(&mut st, Msg::Commit(CommitMsg::SetPushAfterCommit(true)), Instant::now());
    assert!(st.settings.push_after_commit);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.push_after_commit));
    assert!(update(&mut st, Msg::Commit(CommitMsg::SetPushAfterCommit(true)), Instant::now()).is_empty());
}
