//! Headless tests for when the Git Flow and sparse checkout pages say "Saved": only once the
//! page's git job succeeded. While it runs (or waits at the trust prompt) the page counts as
//! saving; a failed job shows git's reason on the page and no tick; a job that is cancelled,
//! dropped at the trust prompt, or ends for another repository leaves nothing behind. The job
//! results are delivered by hand, so the outcome is deterministic.

use super::gitflow_tests::{cleanup, git_job, initialized, job_label, run};
use super::sparse_tests::{cleanup as sparse_cleanup, command, on, open, page_with};
use glowfly_core::sparse_ops::{SparseCommand, SparseMode};
use glowfly_core::trust::ConfigCommand;
use glowfly_model::msg::{JobMsg, Msg, PrefsMsg, SparseMsg};
use glowfly_model::state::job_stop::JobStop;
use glowfly_model::state::prefs_save::{save_error, saving, unsaved};
use glowfly_model::state::{AppState, PrefPage, RunningJob};
use std::path::PathBuf;

/// A repository with Git Flow set up, its page open and the feature prefix edited.
fn edited_flow(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = initialized(name);
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    st.prefs.as_mut().unwrap().gitflow.form.feature = "feat/".into();
    assert!(is_unsaved(&st));
    (st, dir)
}

/// A repository with sparse checkout on, its page open and one more path typed.
fn edited_sparse(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = open(name);
    page_with(&mut st, on(&["src"], SparseMode::Cone));
    st.prefs.as_mut().unwrap().sparse.paths = "src\ndocs".into();
    assert!(is_unsaved(&st));
    (st, dir)
}

fn is_unsaved(st: &AppState) -> bool {
    unsaved(st.prefs.as_ref().unwrap(), &st.settings, st.snap.as_ref())
}

fn save(st: &mut AppState) -> Vec<glowfly_model::effect::Effect> {
    run(st, Msg::Prefs(PrefsMsg::SavePage))
}

fn job_done(st: &mut AppState, ok: bool, output: &str) {
    run(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }));
}

fn tick(st: &AppState) -> bool {
    st.prefs.as_ref().unwrap().saved.pending
}

fn error(st: &AppState) -> Option<String> {
    save_error(st.prefs.as_ref().unwrap()).map(str::to_owned)
}

fn waiting(st: &AppState) -> bool {
    saving(st.prefs.as_ref().unwrap())
}

fn recorded(st: &AppState) -> bool {
    st.prefs.as_ref().unwrap().job_save.is_some()
}

fn nothing_shown(st: &AppState) {
    assert!(!tick(st) && !st.prefs.as_ref().unwrap().saved.showing(), "no tick");
    assert_eq!(error(st), None, "no error");
    assert!(!waiting(st) && !recorded(st), "no pending save");
}

#[test]
fn gitflow_save_shows_saved_only_after_the_job_succeeds() {
    let (mut st, dir) = edited_flow("job-save-ok");
    let fx = save(&mut st);
    assert!(git_job(&fx).is_some(), "{fx:?}");
    assert_eq!(job_label(&st).as_deref(), Some(glowfly_model::state::gitflow::SAVE_LABEL));
    assert!(!tick(&st), "not yet");
    assert!(waiting(&st));
    assert!(git_job(&save(&mut st)).is_none(), "a second Save does nothing while it runs");

    job_done(&mut st, true, "");
    assert!(tick(&st));
    assert_eq!(error(&st), None);
    assert!(!waiting(&st) && !recorded(&st));
    cleanup(&dir);
}

#[test]
fn gitflow_save_failure_shows_the_error_and_never_saved() {
    let (mut st, dir) = edited_flow("job-save-failed");
    save(&mut st);
    job_done(&mut st, false, "$ git config gitflow.prefix.feature feat/\nerror: the test refused this write");
    assert!(!tick(&st) && !st.prefs.as_ref().unwrap().saved.showing());
    assert_eq!(error(&st).as_deref(), Some("error: the test refused this write"));
    assert!(is_unsaved(&st), "the page stays edited");
    assert!(!waiting(&st));

    // Saving again clears the error at once, and the job's success shows the tick.
    let fx = save(&mut st);
    assert!(git_job(&fx).is_some());
    assert_eq!(error(&st), None);
    job_done(&mut st, true, "");
    assert!(tick(&st));
    assert_eq!(error(&st), None);
    cleanup(&dir);
}

#[test]
fn sparse_save_shows_saved_only_after_the_job_succeeds() {
    let (mut st, dir) = edited_sparse("sparse-job-save-ok");
    let fx = save(&mut st);
    assert!(matches!(command(&fx), Some(SparseCommand::Set { .. })), "{fx:?}");
    assert_eq!(job_label(&st).as_deref(), Some(glowfly_model::state::sparse::SAVE_LABEL));
    assert!(!tick(&st));
    assert!(waiting(&st));

    run(&mut st, Msg::Sparse(SparseMsg::Done { ok: true, output: String::new() }));
    assert!(tick(&st));
    assert_eq!(error(&st), None);
    assert!(!waiting(&st) && !recorded(&st));
    sparse_cleanup(&dir);
}

#[test]
fn sparse_save_failure_shows_the_error() {
    let (mut st, dir) = edited_sparse("sparse-job-save-failed");
    save(&mut st);
    run(&mut st, Msg::Sparse(SparseMsg::Done { ok: false, output: "fatal: the test refused these paths".into() }));
    assert!(!tick(&st) && !st.prefs.as_ref().unwrap().saved.showing());
    assert_eq!(error(&st).as_deref(), Some("fatal: the test refused these paths"));
    assert!(is_unsaved(&st), "the page stays edited");
    assert!(!waiting(&st));
    sparse_cleanup(&dir);
}

#[test]
fn a_cancelled_job_save_clears_without_saved_or_error() {
    let (mut st, dir) = edited_flow("job-save-cancelled");
    save(&mut st);
    st.job_stop = Some(JobStop { stopped: true, ..Default::default() });
    run(&mut st, Msg::Job(JobMsg::Stopped { output: String::new(), undo: None }));
    nothing_shown(&st);
    cleanup(&dir);
}

#[test]
fn another_job_does_not_end_the_save() {
    let (mut st, dir) = edited_flow("job-save-other-job");
    save(&mut st);
    assert!(recorded(&st));
    // The save's job is over some other way; an unrelated job ends instead.
    let workdir = st.workdir();
    let mut other = st.job.take().unwrap();
    other.label = "Fetch".into();
    other.workdir = workdir;
    st.job = Some(other);
    job_done(&mut st, true, "");
    assert!(!tick(&st), "a fetch isn't the save");
    assert!(recorded(&st) && waiting(&st), "the save still waits");
    cleanup(&dir);
}

#[test]
fn a_held_job_save_dropped_at_the_trust_prompt_clears() {
    let (mut st, dir) = edited_flow("job-save-held");
    let snap = st.snap.as_mut().unwrap();
    snap.trust_scan.config.push(ConfigCommand { key: "filter.x.clean".into(), value: "x".into() });
    snap.trust_print = snap.trust_scan.print();
    assert!(st.held());

    save(&mut st);
    assert!(st.trust.held.is_some() && st.job.is_none(), "held for the prompt");
    assert!(waiting(&st));

    run(&mut st, Msg::CloseModal);
    assert!(st.trust.held.is_none());
    nothing_shown(&st);
    cleanup(&dir);
}

#[test]
fn a_job_save_that_ends_in_another_tab_shows_nothing() {
    let (mut st, dir) = edited_flow("job-save-other-tab");
    save(&mut st);
    let elsewhere = glowfly_core::paths::temp_dir().join("elsewhere");
    st.prefs.as_mut().unwrap().job_save.as_mut().unwrap().workdir = Some(elsewhere.clone());
    let job: &mut RunningJob = st.job.as_mut().unwrap();
    job.workdir = Some(elsewhere);
    job_done(&mut st, true, "");
    nothing_shown(&st);
    cleanup(&dir);
}
