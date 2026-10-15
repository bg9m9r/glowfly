//! Git Flow: filling the preferences form from the snapshot, Initialize / Save (one git job that
//! creates any missing branch and writes the config), and starting a feature, release or hotfix
//! branch (finishing one is in gitflow_finish.rs). Everything is checked again here against the
//! current snapshot before a job starts.

use super::ops::{JobSpec, busy, start_job};
use crate::effect::Effect;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::gitflow::{INIT_LABEL, SAVE_LABEL, StartForm};
use crate::state::{AppState, Modal, PrefPage};
use std::time::Instant;
use trellis_core::gitflow::{self, Branches, FlowKind, StartProblem};

pub(super) const NOT_SET_UP: &str = "Git Flow isn't set up in this repository. Set it up in Preferences > Git Flow.";

pub(super) fn handle(st: &mut AppState, m: GitFlowMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        GitFlowMsg::ShowPage => {
            if let Some(p) = st.prefs.as_mut() {
                p.page = PrefPage::RepoGitFlow;
                fill(st);
            }
        }
        GitFlowMsg::Apply => apply(st, fx),
        GitFlowMsg::Discard => {
            if let Some(p) = st.prefs.as_mut() {
                p.gitflow.form = p.gitflow.filled.clone();
                p.gitflow.error = None;
            }
        }
        GitFlowMsg::StartDialog(kind) => start_dialog(st, kind),
        GitFlowMsg::Start { kind, name } => start(st, kind, &name, now, fx),
        GitFlowMsg::FinishDialog(b) => super::gitflow_finish::dialog(st, &b),
        GitFlowMsg::Finish { branch, options } => super::gitflow_finish::finish(st, &branch, options, now, fx),
    }
}

/// Fills the open page's form from the snapshot.
pub(super) fn fill(st: &mut AppState) {
    if let (Some(prefs), Some(snap)) = (st.prefs.as_mut(), st.snap.as_ref()) {
        prefs.gitflow.fill(snap);
    }
}

/// A new snapshot arrived: refill the open page when it belongs to another repository, or when
/// the config changed and the form wasn't edited (an edited form keeps the user's values) or now
/// matches it (a successful Save).
pub(super) fn snapshot_changed(st: &mut AppState) {
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

fn apply(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(prefs), Some(snap)) = (st.prefs.as_mut(), st.snap.as_ref()) else { return };
    let page = &mut prefs.gitflow;
    let form = page.form.trimmed();
    if let Err(p) = gitflow::check_settings(&form) {
        page.error = Some(p.message());
        return;
    }
    let cmds = match gitflow::init_cmds(&form, &snap.gitflow, &Branches::of(snap)) {
        Ok(cmds) => cmds,
        Err(p) => {
            page.error = Some(p.message());
            return;
        }
    };
    page.error = None;
    let label = if snap.gitflow.is_initialized() { SAVE_LABEL } else { INIT_LABEL };
    start_job(st, JobSpec::new(label.into(), cmds), fx);
}

fn start_dialog(st: &mut AppState, kind: FlowKind) {
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
    let name = gitflow::suggest_version(&b.tags, &s.version_tag, kind).unwrap_or_default();
    st.modal = Some(Modal::GitFlowStart(StartForm { kind, name }));
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
