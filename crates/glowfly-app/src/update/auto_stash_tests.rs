//! Stash and carry on: when the prompt asks and what it offers, Always stash, the hard reset's
//! Discard and reset, and when a pending auto-stash may go back (stale loads, a running job,
//! another tab).

use super::tests::{clean_tree, drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, StashEffect};
use crate::msg::{AutoStashMsg, JobMsg, Msg, RepoMsg, TabsMsg};
use crate::state::auto_stash::{AutoStashAsk, AutoStashJob, PendingStash, Phase, button};
use crate::state::{AppState, Modal, RunningJob, Selection};
use git2::Oid;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::stash_ops::AutoStashFor;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn ask(st: &AppState) -> &AutoStashAsk {
    match &st.modal {
        Some(Modal::AutoStash(a)) => a,
        _ => panic!("expected the auto-stash prompt"),
    }
}

/// The commands of the one RunGit in `fx`.
pub(super) fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).collect::<Vec<_>>()[..] {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        _ => panic!("expected one RunGit, got {fx:?}"),
    }
}

/// A clean repository on main with a branch `feature` at the same commit, opened.
fn clean_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    git(&dir, &["branch", "feature"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn job(label: &str, workdir: Option<PathBuf>) -> RunningJob {
    RunningJob { label: label.into(), workdir, ..super::rebase_tests::running_job(Path::new(".")) }
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn checkout_feature() -> Msg {
    Msg::git("Checkout", &["checkout", "feature", "--"])
}

fn stash_oids(dir: &Path) -> Vec<Oid> {
    glowfly_core::stash_ops::list(dir).unwrap().into_iter().map(|s| s.oid).collect()
}

#[test]
fn clean_tree_never_asks() {
    let (mut st, dir) = clean_repo("as-clean");
    for msg in [checkout_feature(), Msg::git("Merge", &["merge", "feature"]), Msg::git("Pull", &["pull", "--ff"])] {
        let fx = up(&mut st, msg);
        assert_eq!(git_cmds(&fx).len(), 1, "{fx:?}");
        assert!(st.modal.is_none());
        st.job = None;
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dirty_tree_asks_naming_operation_and_count() {
    let (mut st, dir) = clean_repo("as-asks");
    std::fs::write(dir.join("new.txt"), "untracked\n").unwrap();
    refresh(&mut st);
    let fx = up(&mut st, checkout_feature());
    assert_eq!(git_cmds(&fx).len(), 1, "untracked files don't count");
    st.job = None;

    git(&dir, &["add", "new.txt"]);
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    refresh(&mut st);
    assert!(up(&mut st, checkout_feature()).is_empty());
    let a = ask(&st);
    assert_eq!(a.op, AutoStashFor::Checkout { target: "feature".into() });
    assert_eq!(a.files, 2);
    assert!(!a.always);
    assert_eq!(button(Lang::En, &a.op), "Stash and check out");
    let body = crate::state::auto_stash::body(Lang::En, &a.op, a.files);
    assert_eq!(body, "You have 2 changed files. Glowfly stashes them, checks out feature and puts them back afterwards.");

    up(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && st.job.is_none());
    assert!(st.auto_stash.making.is_none() && st.auto_stash.pending.is_empty());
    assert!(stash_oids(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_busy_app_or_another_dialog_opens_no_prompt() {
    let (mut st, dir) = clean_repo("as-busy");
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    refresh(&mut st);
    st.job = Some(job("Fetch", st.workdir()));
    assert!(up(&mut st, checkout_feature()).is_empty());
    assert!(st.modal.is_none());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Another operation is still running."));
    st.job = None;
    st.modal = Some(Modal::About);
    assert!(up(&mut st, checkout_feature()).is_empty());
    assert!(matches!(st.modal, Some(Modal::About)), "nothing is replaced");
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Close the open dialog first."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hard_reset_asks_with_discard() {
    let (mut st, dir) = clean_repo("as-hard");
    let first = st.snap.as_ref().unwrap().head_oid.unwrap().to_string();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "add b"]);
    std::fs::write(dir.join("a.txt"), "one\nedited\n").unwrap();
    refresh(&mut st);
    let reset = || Msg::git("Reset (hard)", &["reset", "--hard", &first]);

    for (mode, label) in [("--soft", "Reset (soft)"), ("--mixed", "Reset (mixed)")] {
        let fx = up(&mut st, Msg::git(label, &["reset", mode, &first]));
        assert_eq!(git_cmds(&fx).len(), 1, "{mode} never asks");
        assert!(st.modal.is_none());
        st.job = None;
    }

    st.settings.auto_stash = true;
    assert!(up(&mut st, reset()).is_empty(), "a hard reset asks even with Always stash");
    assert!(ask(&st).op.is_hard_reset());
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Discard));
    assert_eq!(git_cmds(&fx), glowfly_core::undo::hard_reset_commands(&first));
    assert!(st.modal.is_none());
    st.job = None;

    assert!(up(&mut st, reset()).is_empty());
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    let message = format!("Auto-stash before resetting main to {}", &first[..7]);
    assert_eq!(git_cmds(&fx), vec![args(&["stash", "push", "-m", &message]), args(&["reset", "--hard", &first])]);
    super::tests::finish(&mut st, fx);
    assert!(!dir.join("b.txt").exists(), "the reset ran");
    let a = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(a, "one\nedited\n", "the changes came back");
    assert!(stash_oids(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `st` with a pending auto-stash for its tab, a real stash with that message, and the given
/// arming generation.
fn pending(st: &mut AppState, dir: &Path, from: u64) -> String {
    let op = AutoStashFor::Checkout { target: "feature".into() };
    let message = op.message();
    std::fs::write(dir.join("a.txt"), "one\nstashed\n").unwrap();
    git(dir, &["stash", "push", "-q", "-m", &message]);
    let job = AutoStashJob { op, message: message.clone(), before: Vec::new() };
    let workdir = st.workdir().unwrap();
    st.auto_stash.pending.push(PendingStash { workdir, job, oid: None, phase: Phase::Ended { from }, told: false });
    message
}

fn put_backs(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Stash(StashEffect::PutBack(_)))).count()
}

#[test]
fn stale_load_never_puts_back() {
    let (mut st, dir) = clean_repo("as-stale");
    let from = st.load_gen + 1;
    pending(&mut st, &dir, from);
    let snap = glowfly_core::repo::load(&dir, &glowfly_core::repo::LoadOptions::all(100)).unwrap();
    let old =
        Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: crate::effect::LoadKind::Refresh, result: Ok(Box::new(snap)) });
    st.loading = true;
    let fx = up(&mut st, old);
    assert_eq!(put_backs(&fx), 0, "a load from before the job ended: {fx:?}");
    assert!(st.auto_stash.pending[0].oid.is_some(), "the stash is found");
    refresh(&mut st);
    assert!(st.auto_stash.pending.is_empty());
    assert!(stash_oids(&dir).is_empty(), "a newer load put it back");
    let a = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(a, "one\nstashed\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn put_back_waits_for_running_job_and_other_tab() {
    let (mut st, dir) = clean_repo("as-wait");
    let from = st.load_gen + 1;
    pending(&mut st, &dir, from);
    st.job = Some(job("Fetch", st.workdir()));
    let fx = up(&mut st, Msg::Repo(RepoMsg::Refresh));
    super::tests::finish(&mut st, fx);
    assert_eq!(stash_oids(&dir).len(), 1, "waits while a job runs");
    assert_eq!(st.auto_stash.pending.len(), 1);
    st.job = None;

    // Another tab is active: nothing happens until this one is active and loaded again.
    let other = scratch_repo("as-wait-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    refresh(&mut st);
    assert_eq!(stash_oids(&dir).len(), 1);
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(stash_oids(&dir).is_empty(), "put back once its tab loaded");
    assert!(st.auto_stash.pending.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn show_stash_opens_section_and_selects() {
    let (mut st, dir) = clean_repo("as-show");
    std::fs::write(dir.join("a.txt"), "x\n").unwrap();
    git(&dir, &["stash", "push", "-q", "-m", "Auto-stash before checking out feature"]);
    refresh(&mut st);
    let oid = stash_oids(&dir)[0];
    st.view.sections.stashes = false;
    drive(&mut st, Msg::AutoStash(AutoStashMsg::Show(oid)));
    assert!(st.view.sections.stashes);
    assert_eq!(st.selection, Selection::Commit(oid));
    up(&mut st, Msg::AutoStash(AutoStashMsg::Show(Oid::from_bytes(&[3; 20]).unwrap())));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("That stash is no longer in the list."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn held_job_drops_its_mark() {
    let (mut st, dir) = super::trust_held_tests::held_repo_with("as-held", false);
    git(&dir, &["branch", "feature"]);
    refresh(&mut st);
    assert!(up(&mut st, checkout_feature()).is_empty());
    up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert!(st.trust.held.is_some(), "the trust prompt holds the job");
    assert!(st.auto_stash.making.is_some());
    up(&mut st, Msg::CloseModal);
    assert!(st.trust.held.is_none());
    assert!(st.auto_stash.making.is_none(), "dropped with the held job");
    st.job = Some(job("Checkout", st.workdir()));
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(st.auto_stash.pending.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn interactive_rebase_with_always_stash_skips_question() {
    use crate::effect::RebaseEffect;
    use crate::msg::RebaseMsg;
    use glowfly_core::rebase::{RebaseAction, RebaseBase};
    let (mut st, dir, _) = super::rebase_tests::with_panel("as-rebase", RebaseBase::FromCommit(Oid::zero()));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    std::fs::write(dir.join("a.txt"), "one\nedited\n").unwrap();
    refresh(&mut st);
    st.settings.auto_stash = true;
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    assert!(st.modal.is_none(), "no stash question");
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::Start { request, .. })] if request.autostash), "{fx:?}");
    st.job = None;
    st.rebase.as_mut().unwrap().starting = false;

    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Drop }));
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false })).is_empty());
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })), "dropping commits still asks");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gate_records_the_retry_of_a_guarded_command() {
    use super::auto_stash::{Gate, gate};
    let (mut st, dir) = clean_repo("as-gate-retry");
    let cmd = args(&["checkout", "feature", "--"]);
    let want = AutoStashFor::Checkout { target: "feature".into() };
    let g = gate(&mut st, &cmd, checkout_feature);
    assert!(
        matches!(&g, Gate::Run(Some(r)) if r.op == want && matches!(*r.retry, Msg::Job(JobMsg::Git { ref label, .. }) if label == "Checkout"))
    );
    assert!(matches!(gate(&mut st, &args(&["fetch"]), || Msg::git("Fetch", &["fetch"])), Gate::Run(None)), "not guarded");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn approval_beats_a_snapshot_that_shows_no_changes() {
    let (mut st, dir) = clean_repo("as-gate-approved");
    st.auto_stash.approved =
        Some(crate::state::auto_stash::Approval { op: AutoStashFor::Checkout { target: "feature".into() }, stash: true });
    let fx = up(&mut st, checkout_feature());
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out feature"]), args(&["checkout", "feature", "--"])]
    );
    assert!(st.auto_stash.approved.is_none(), "taken by the gate");
    let _ = std::fs::remove_dir_all(&dir);
}
