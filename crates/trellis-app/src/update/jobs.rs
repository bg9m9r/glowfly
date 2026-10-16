//! Starting the single active job: busy check, the trust gate (update/trust.rs), and the
//! RunningJob plus effect it runs.

use crate::effect::Effect;
use crate::state::trust::Created;
use crate::state::{AppState, RunningJob};
use std::path::PathBuf;
use trellis_core::undo::UndoKind;

/// A git command sequence to run as the (single) active job.
pub(super) struct JobSpec {
    pub label: String,
    pub cmds: Vec<Vec<String>>,
    pub cwd: Option<PathBuf>,
    pub quiet: bool,
    pub clear_commit: bool,
    /// Push the branch once the job (a commit) succeeds.
    pub then_push: bool,
    /// Open this repository in the active tab once the job succeeds; how it was made decides
    /// whether it is trusted (update/trust.rs).
    pub then_open: Option<(PathBuf, Created)>,
    pub write_file: Option<(PathBuf, String)>,
    /// Record the job for undo when it is this kind of operation.
    pub record: Option<UndoKind>,
    /// Run nothing unless this revision still resolves to this commit.
    pub expect: Option<(String, git2::Oid)>,
}

impl JobSpec {
    pub fn new(label: String, cmds: Vec<Vec<String>>) -> Self {
        Self {
            label,
            cmds,
            cwd: None,
            quiet: false,
            clear_commit: false,
            then_push: false,
            then_open: None,
            write_file: None,
            record: None,
            expect: None,
        }
    }
}

/// What became of a job that was asked to start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Started {
    /// It is the active job and its effect was pushed.
    Now,
    /// It waits for the trust prompt; no effect was pushed.
    Held,
    /// Busy, no repository, or dropped; already logged.
    No,
}

/// Whether a job is already running (only one runs at a time); tells the user unless `quiet`.
pub(super) fn busy(st: &mut AppState, quiet: bool) -> bool {
    if st.job.is_some() && !quiet {
        st.log(false, "Another operation is still running.");
    }
    st.job.is_some()
}

/// Runs `spec` as the active job. Holding it for the trust prompt pushes no effect.
pub(super) fn start_job(st: &mut AppState, spec: JobSpec, fx: &mut Vec<Effect>) -> Started {
    let job = RunningJob {
        label: spec.label,
        workdir: None,
        quiet: spec.quiet,
        clear_commit: spec.clear_commit,
        then_push: spec.then_push,
        then_open: spec.then_open,
        keeps_undo: super::undo::keeps_history(&spec.cmds),
    };
    // Background jobs fail rather than pop up a credential prompt.
    let prompts = !spec.quiet;
    let (cmds, write_file, record, expect) = (spec.cmds, spec.write_file, spec.record, spec.expect);
    launch(st, job, spec.cwd, move |cwd| Effect::RunGit { cwd, cmds, write_file, record, prompts, expect }, fx)
}

/// Makes a job labelled `label` the active one in the active tab's repository, running the
/// effect `effect` builds for that folder, unless another job is running (said in the log).
/// Holding it for the trust prompt pushes no effect, so callers that close their dialog only
/// when an effect was pushed keep it open.
pub(super) fn begin_job(
    st: &mut AppState,
    label: &str,
    keeps_undo: bool,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    let job =
        RunningJob { label: label.into(), workdir: None, quiet: false, clear_commit: false, then_push: false, then_open: None, keeps_undo };
    launch(st, job, None, effect, fx)
}

/// Makes `job` the active job (in the active tab) and pushes its effect for `cwd`, or the
/// active tab's repository, unless one is running or git is held there (update/trust.rs).
fn launch(
    st: &mut AppState,
    mut job: RunningJob,
    cwd: Option<PathBuf>,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    if busy(st, job.quiet) {
        return Started::No;
    }
    let Some(cwd) = cwd.or_else(|| st.workdir()) else { return Started::No };
    job.workdir = st.workdir();
    if super::trust::waits(st, &job, &cwd) {
        return super::trust::hold(st, job, effect(cwd));
    }
    st.job = Some(job);
    fx.push(effect(cwd));
    Started::Now
}
