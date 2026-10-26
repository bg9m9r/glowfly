//! Git Flow: filling the preferences form from the snapshot, Initialize / Save (one git job that
//! creates any missing branch and writes the config), starting a feature, release or hotfix
//! branch (also with an issue's branch name typed), and opening and closing the Git Flow panel
//! (finishing a branch is in gitflow_finish.rs, continuing one stopped at a conflict in
//! gitflow_resume.rs). Everything is checked again here against the current snapshot before a
//! job starts.

use super::ops::{JobSpec, Started, busy, start_job};
use crate::effect::Effect;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::gitflow::{INIT_LABEL, NOT_SET_UP, SAVE_LABEL, StartForm};
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::gitflow::{self, Branches, FlowKind, StartProblem};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, m: GitFlowMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        GitFlowMsg::ShowPage => {
            if let Some(p) = st.prefs.as_mut() {
                p.page = PrefPage::RepoGitFlow;
                fill(st);
            }
        }
        GitFlowMsg::Apply => {
            apply(st, fx);
        }

        GitFlowMsg::StartDialog(kind) => start_dialog(st, kind, None),
        GitFlowMsg::StartFromIssue { kind, name } => start_dialog(st, kind, Some(name)),
        GitFlowMsg::Start { kind, name } => start(st, kind, &name, now, fx),
        GitFlowMsg::FinishDialog(b) => super::gitflow_finish::dialog(st, &b),
        GitFlowMsg::Finish { branch, options } => super::gitflow_finish::finish(st, &branch, options, false, now, fx),
        GitFlowMsg::StashAndFinish { branch, options } => super::gitflow_finish::finish(st, &branch, options, true, now, fx),
        GitFlowMsg::OpenPanel => open_panel(st),
        GitFlowMsg::ClosePanel => st.session.gitflow_panel = false,
        GitFlowMsg::ContinueFinish => super::gitflow_resume::continue_finish(st, now, fx),
        GitFlowMsg::ForgetFinish => {
            st.session.gitflow_resume = None;
            // A stash held for the forgotten steps comes back at once.
            super::auto_stash::seen(st, false, now, fx);
        }
    }
}

/// Fills the open page's form from the snapshot.
pub(super) fn fill(st: &mut AppState) {
    if let (Some(prefs), Some(snap)) = (st.prefs.as_mut(), st.snap.as_ref()) {
        prefs.gitflow.fill(snap);
    }
}

/// A new snapshot arrived: follow a stopped Finish (gitflow_resume.rs), and refill the open page
/// when it belongs to another repository, or when the config changed and the form wasn't edited
/// (an edited form keeps the user's values) or now matches it (a successful Save).
pub(super) fn snapshot_changed(st: &mut AppState) {
    super::gitflow_resume::snapshot_changed(st);
    let (Some(prefs), Some(snap)) = (st.prefs.as_mut(), st.snap.as_ref()) else { return };
    let page = &mut prefs.gitflow;
    if page.workdir.as_ref() != Some(&snap.workdir) {
        page.fill(snap);
    } else if snap.gitflow != page.base {
        let saved = snap.gitflow.settings().is_some_and(|s| s == page.form.trimmed());
        if page.edited() && !saved {
            page.base = snap.gitflow.clone();
        } else {
            page.fill(snap);
        }
    }
}

/// Puts the form back to what it was filled with.
pub(super) fn discard(st: &mut AppState) {
    if let Some(p) = st.prefs.as_mut() {
        p.gitflow.form = p.gitflow.filled.clone();
        p.gitflow.error = None;
    }
}

/// Initialize / Save: checks the form and starts the job; false when it was refused (the
/// problem is on the page, or another job runs).
pub(super) fn apply(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let (Some(prefs), Some(snap)) = (st.prefs.as_mut(), st.snap.as_ref()) else { return false };
    let page = &mut prefs.gitflow;
    let form = page.form.trimmed();
    if let Err(p) = gitflow::check_settings(&form) {
        page.error = Some(p.message());
        return false;
    }
    let cmds = match gitflow::init_cmds(&form, &snap.gitflow, &Branches::of(snap)) {
        Ok(cmds) => cmds,
        Err(p) => {
            page.error = Some(p.message());
            return false;
        }
    };
    page.error = None;
    let label = if snap.gitflow.is_initialized() { SAVE_LABEL } else { INIT_LABEL };
    start_job(st, JobSpec::new(label.into(), cmds), fx) != Started::No
}

/// Shows the Git Flow panel in place of the graph, closing the diff, the conflict view, the
/// file history and the interactive rebase panel; nothing while a rebase is starting.
fn open_panel(st: &mut AppState) {
    if st.snap.is_none() || st.session.rebase.view.as_ref().is_some_and(|p| p.starting) {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    super::rebase::close(st);
    super::journal::close(st);
    super::range_diff::close(st);
    st.session.gitflow_panel = true;
}

/// Opens the Start dialog for `kind` with `given` typed after the prefix, else the next version
/// for a release or hotfix.
fn start_dialog(st: &mut AppState, kind: FlowKind, given: Option<String>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    let b = Branches::of(snap);
    let base = s.base(kind);
    if !b.local(base) {
        let problem = StartProblem::MissingBase { kind, base: base.to_owned() };
        st.log(false, problem.message());
        return;
    }
    let name = given.unwrap_or_else(|| gitflow::suggest_version(&b.tags, &s.version_tag, kind).unwrap_or_default());
    st.modal = Some(Modal::GitFlowStart(StartForm { kind, name, touched: Default::default() }));
}

fn start(st: &mut AppState, kind: FlowKind, name: &str, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(s) = snap.gitflow.settings() else {
        st.log(false, NOT_SET_UP);
        return;
    };
    let branch = match gitflow::check_start(&s, kind, name, &Branches::of(snap)) {
        Ok(branch) => branch,
        Err(p) => {
            st.log(false, p.message());
            return;
        }
    };
    // Keep the dialog (and the typed name) when the job can't start.
    if busy(st, false) {
        return;
    }
    if matches!(st.modal, Some(Modal::GitFlowStart(_))) {
        st.modal = None;
    }
    if st.prefs.is_some() {
        fx.extend(super::update(st, Msg::Prefs(PrefsMsg::ClosePrefs), now));
    }
    let cmd = gitflow::start_cmd(&s, kind, &branch);
    start_job(st, JobSpec::new(format!("Start {branch}"), vec![cmd]), fx);
}
