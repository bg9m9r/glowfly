//! Headless tests for batch submodule updates: Initialize all and Update all (asking before
//! leaving commits behind), the update after a checkout or rebase and when it is left out, the
//! offer to initialize on open, and the per-repository choice.

use super::submodule_tests::{PATH, cleanup, commit_detached, deinit, last_log, opened, sub};
use super::tests::{drive, finish, git, skipped, state};
use super::update;
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{Msg, RepoMsg};
use crate::runner;
use crate::state::submodules::batch::CONFIRM_ALL_BUTTON;
use crate::state::{AppState, Modal};
use crate::submodule::{Batch, SubmoduleMsg};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::SubmoduleState;
use trellis_core::settings::{RepoTrust, SubmoduleUpdates};
use trellis_core::submodule_ops::batch_update_cmds;

fn batch(batch: Batch, confirmed: bool) -> Msg {
    Msg::Submodule(SubmoduleMsg::Batch { batch, confirmed })
}

/// Runs the effects `finish` would run, one step: the effects their results lead to come back
/// instead of running.
fn step(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let mut out = Vec::new();
    for e in fx.into_iter().filter(|e| !skipped(e)) {
        if let Some(m) = runner::execute(e) {
            out.extend(update(st, m, Instant::now()));
        }
    }
    out
}

fn checks_all(fx: &[Effect], want: Batch) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Submodule(SubmoduleEffect::CheckAll { batch, .. }) if *batch == want))
}

/// The commands of the git job in `fx` labelled `label`, if one was started.
fn job_cmds(st: &AppState, fx: &[Effect], label: &str) -> Option<Vec<Vec<String>>> {
    if st.job.as_ref().map(|j| j.label.as_str()) != Some(label) {
        return None;
    }
    fx.iter().find_map(|e| match e {
        Effect::RunGit { cmds, .. } => Some(cmds.clone()),
        _ => None,
    })
}

fn real(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().real_workdir.clone()
}

#[test]
fn init_all_runs_one_job() {
    let (mut st, parent, lib) = opened("smb-init-all");
    deinit(&mut st, &parent);
    let fx = update(&mut st, batch(Batch::InitAll, false), Instant::now());
    match &fx[..] {
        [Effect::Submodule(e @ SubmoduleEffect::CheckAll { batch: Batch::InitAll, .. })] => assert!(e.scope().is_some()),
        other => panic!("{other:?}"),
    }
    let m = runner::execute(fx.into_iter().next().unwrap()).unwrap();
    let Msg::Scoped { msg, .. } = &m else { panic!("expected a scoped result") };
    assert!(matches!(&**msg, Msg::Submodule(SubmoduleMsg::CheckedAll { batch: Batch::InitAll, result: Ok(_) })));
    let fx = update(&mut st, m, Instant::now());
    assert_eq!(job_cmds(&st, &fx, "Initialize submodules"), Some(batch_update_cmds(None, true)));
    finish(&mut st, fx);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Initialize submodules succeeded"));
    cleanup(&[&parent, &lib]);
}

#[test]
fn update_all_asks_when_commits_would_be_stranded() {
    let (mut st, parent, lib) = opened("smb-update-all");
    let detached = commit_detached(&parent, "detached\n");
    let fx = update(&mut st, batch(Batch::UpdateAll, false), Instant::now());
    assert!(checks_all(&fx, Batch::UpdateAll), "{fx:?}");
    let fx = step(&mut st, fx);
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    assert_eq!(sub(&st).left_behind.len(), 1, "the row was read again");
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, then, .. }) => {
            assert_eq!(title, "Update all submodules");
            assert!(body.contains(&format!("{PATH}: 1 commit")) && body.contains(&detached.to_string()[..7]), "{body}");
            assert_eq!(button, CONFIRM_ALL_BUTTON);
            assert!(matches!(&**then, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::UpdateAll, confirmed: true })));
        }
        _ => panic!("expected a confirmation"),
    }
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert_eq!(job_cmds(&st, &fx, "Update submodules"), Some(batch_update_cmds(None, true)));
    finish(&mut st, fx);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);

    // Nothing left to do: said in the log, no job.
    let fx = update(&mut st, batch(Batch::UpdateAll, false), Instant::now());
    let fx = step(&mut st, fx);
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    assert_eq!(last_log(&st), (true, "Every submodule is at its recorded commit already.".to_owned()));
    cleanup(&[&parent, &lib]);
}

/// `opened`, plus a branch `other` that records a newer commit of the submodule; main is
/// checked out with the submodule in sync.
fn two_branches(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, parent, lib) = opened(name);
    let dir = parent.join(PATH);
    std::fs::write(dir.join("a.txt"), "other\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "other"]);
    git(&parent, &["checkout", "-q", "-b", "other"]);
    git(&parent, &["add", PATH]);
    git(&parent, &["commit", "-q", "-m", "lib on other"]);
    git(&parent, &["checkout", "-q", "main"]);
    git(&parent, &["submodule", "update", "-q", "--checkout", "--", PATH]);
    super::tests::clean_tree(&parent);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    (st, parent, lib)
}

fn checkout_other() -> Msg {
    Msg::git("Checkout", &["checkout", "other", "--"])
}

/// Starts `msg`'s job and runs it: the effects its end leads to.
fn after_job(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let fx = update(st, msg, Instant::now());
    step(st, fx)
}

#[test]
fn auto_update_after_checkout() {
    let (mut st, parent, lib) = two_branches("smb-auto");
    st.settings.submodules.keep_updated = true;
    let fx = after_job(&mut st, checkout_other());
    assert!(checks_all(&fx, Batch::Auto), "{fx:?}");
    let fx = step(&mut st, fx);
    assert_eq!(job_cmds(&st, &fx, "Update submodules"), Some(batch_update_cmds(None, false)), "no --checkout");
    finish(&mut st, fx);
    let sm = sub(&st);
    assert_eq!(sm.state, SubmoduleState::InSync);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("other"));
    cleanup(&[&parent, &lib]);
}

#[test]
fn no_auto_update_unless_wanted() {
    let (mut st, parent, lib) = two_branches("smb-auto-off");
    assert!(!checks_all(&after_job(&mut st, checkout_other()), Batch::Auto), "the setting is off");
    git(&parent, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    st.settings.submodules.keep_updated = true;
    let r = real(&st);
    st.settings.set_submodule_updates(&r, Some(false));
    assert!(!checks_all(&after_job(&mut st, checkout_other()), Batch::Auto), "Never in this repository");
    git(&parent, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    st.settings.submodules.keep_updated = false;
    st.settings.set_submodule_updates(&r, Some(true));
    assert!(checks_all(&after_job(&mut st, checkout_other()), Batch::Auto), "Always in this repository");
    cleanup(&[&parent, &lib]);
}

#[test]
fn no_auto_update_after_other_jobs() {
    let (mut st, parent, lib) = two_branches("smb-auto-jobs");
    st.settings.submodules.keep_updated = true;
    let failed = after_job(&mut st, Msg::git("Checkout", &["checkout", "no-such-branch", "--"]));
    assert!(!checks_all(&failed, Batch::Auto), "the job failed");
    let commit = after_job(&mut st, Msg::git("Commit", &["commit", "-q", "--allow-empty", "-m", "x"]));
    assert!(!checks_all(&commit, Batch::Auto), "a commit doesn't move the work tree");
    assert!(st.submodules.after_job.is_none());

    // A stale mark (a job held for trust and then declined) never fires for another job.
    let fx = update(&mut st, checkout_other(), Instant::now());
    let w = st.workdir().unwrap();
    st.submodules.after_job = Some((w, "Pull".into()));
    assert!(!checks_all(&step(&mut st, fx), Batch::Auto), "another label");
    cleanup(&[&parent, &lib]);
}

#[test]
fn no_auto_update_after_switching_tabs() {
    let (mut st, parent, lib) = two_branches("smb-auto-tab");
    st.settings.submodules.keep_updated = true;
    let job = update(&mut st, checkout_other(), Instant::now());
    let mut fx = Vec::new();
    super::tabs::open(&mut st, lib.clone(), &mut fx);
    finish(&mut st, fx);
    assert!(st.workdir().is_some_and(|w| !trellis_core::paths::same_spelling(&w, &parent)));
    assert!(!checks_all(&step(&mut st, job), Batch::Auto), "the tab changed before the job ended");
    cleanup(&[&parent, &lib]);
}

#[test]
fn no_auto_update_in_an_untrusted_repository() {
    let (mut st, parent, lib) = two_branches("smb-auto-untrusted");
    st.settings.submodules.keep_updated = true;
    let job = update(&mut st, checkout_other(), Instant::now());
    let r = real(&st);
    trellis_core::settings::repo_keys::set(&mut st.settings.repo_trust, &r, Some(RepoTrust::NotTrusted));
    assert!(!st.runs_git_unasked());
    let fx = step(&mut st, job);
    let fx = step(&mut st, fx);
    assert!(st.job.as_ref().is_none_or(|j| j.label != "Update submodules"), "{fx:?}");
    cleanup(&[&parent, &lib]);
}

#[test]
fn auto_skips_stranding_submodule() {
    let (mut st, parent, lib) = opened("smb-auto-strand");
    st.settings.submodules.keep_updated = true;
    commit_detached(&parent, "detached\n");
    let fx = update(&mut st, batch(Batch::Auto, false), Instant::now());
    let fx = step(&mut st, fx);
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    let (_, text) = last_log(&st);
    assert!(text.contains(PATH) && text.contains("1 commit") && text.contains("left as it is"), "{text}");
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync);
    cleanup(&[&parent, &lib]);
}

fn offers(st: &AppState) -> usize {
    st.toasts.iter().filter(|t| t.action.as_ref().is_some_and(|a| a.label == "Initialize")).count()
}

fn open_in(st: &mut AppState, dir: &Path) {
    drive(st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
}

#[test]
fn open_offers_to_initialize_once() {
    let (st, parent, lib) = opened("smb-offer");
    assert_eq!(offers(&st), 0, "everything is checked out");
    git(&parent, &["submodule", "deinit", "-q", "-f", PATH]);

    let mut st = state();
    open_in(&mut st, &parent);
    assert_eq!(offers(&st), 1);
    let t = st.toasts.iter().find(|t| t.action.is_some()).unwrap();
    assert_eq!(t.text, format!("1 submodule in {} isn't checked out yet.", st.snap.as_ref().unwrap().name));
    let action = t.action.as_ref().unwrap();
    assert!(action.workdir.as_deref().is_some_and(|w| trellis_core::paths::same_spelling(w, &st.workdir().unwrap())));
    assert!(matches!(&*action.msg, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: true })));
    open_in(&mut st, &parent);
    assert_eq!(offers(&st), 1, "once per session");
    let m = action_msg(&mut st);
    drive(&mut st, *m);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);

    // Keeping submodules updated initializes them at once instead.
    git(&parent, &["submodule", "deinit", "-q", "-f", PATH]);
    let mut st = state();
    st.settings.submodules.keep_updated = true;
    open_in(&mut st, &parent);
    assert_eq!(offers(&st), 0);
    assert!(
        st.toasts.iter().any(|t| t.text == "Initialize submodules succeeded"),
        "{:?}",
        st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    cleanup(&[&parent, &lib]);
}

/// Takes the offer toast's action message, as clicking its button would.
fn action_msg(st: &mut AppState) -> Box<Msg> {
    st.toasts.iter_mut().find_map(|t| t.action.take()).map(|a| a.msg).unwrap()
}

#[test]
fn per_repository_choice_is_saved() {
    let (mut st, parent, lib) = opened("smb-choice");
    let r = real(&st);
    let fx = update(&mut st, Msg::Submodule(SubmoduleMsg::SetUpdates(SubmoduleUpdates::Always)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "{fx:?}");
    assert_eq!(st.settings.submodule_updates_for(&r), Some(true));
    assert_eq!(crate::state::submodules::section_cx(&st).updates, SubmoduleUpdates::Always);
    update(&mut st, Msg::Submodule(SubmoduleMsg::SetUpdates(SubmoduleUpdates::FollowGlobal)), Instant::now());
    assert!(st.settings.repo_submodule_updates.is_empty());

    // A batch's answer for a tab that is no longer active is dropped.
    let scoped = Msg::Scoped {
        workdir: lib.clone(),
        msg: Box::new(Msg::Submodule(SubmoduleMsg::CheckedAll { batch: Batch::InitAll, result: Ok(Vec::new()) })),
    };
    assert!(update(&mut st, scoped, Instant::now()).is_empty());
    assert!(st.job.is_none() && st.modal.is_none());
    cleanup(&[&parent, &lib]);
}

#[test]
fn a_rebase_is_followed_by_an_update() {
    let (mut st, parent, lib) = opened("smb-rebase");
    st.settings.submodules.keep_updated = true;
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let request = trellis_core::rebase::StartRequest {
        head,
        upstream: trellis_core::rebase::Upstream::Commit(head),
        items: Vec::new(),
        autostash: false,
    };
    let mut fx = Vec::new();
    assert_eq!(super::rebase::launch(&mut st, "Rebase", request, "rebase of main".into(), &mut fx), super::ops::Started::Now);
    assert_eq!(st.submodules.after_job.as_ref().map(|(_, l)| l.as_str()), Some("Rebase"));
    let mut fx = Vec::new();
    super::ops::finish_job(&mut st, true, String::new(), None, None, Instant::now(), &mut fx);
    assert!(checks_all(&fx, Batch::Auto), "{fx:?}");
    cleanup(&[&parent, &lib]);
}
