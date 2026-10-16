//! Starting the single active job: busy check, the trust gate (update/trust.rs), and the
//! RunningJob plus effect it runs (`begin_tool_job` for the merge tool, which skips the gate).

use crate::effect::Effect;
use crate::state::push_pr::PrStart;
use crate::state::trust::Created;
use crate::state::{AppState, RunningJob};
use std::path::PathBuf;
use std::time::Instant;
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
    /// Open the create pull request dialog once the job (a push) succeeds.
    pub then_pr: Option<PrStart>,
    /// Check the repository again for a pending link's target once the job (a fetch) ends.
    pub then_link: bool,
    /// Open this repository in the active tab once the job succeeds; how it was made decides
    /// whether it is trusted (update/trust.rs).
    pub then_open: Option<(PathBuf, Created)>,
    pub write_file: Option<(PathBuf, String)>,
    /// Record the job for undo when it is this kind of operation. Left `None`, a single command
    /// run in the active tab's repository is recognised by `UndoKind::classify`.
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
            then_pr: None,
            then_link: false,
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
        then_pr: spec.then_pr,
        then_link: spec.then_link,
        then_open: spec.then_open,
        keeps_undo: super::undo::keeps_history(&spec.cmds),
    };
    // Background jobs fail rather than pop up a credential prompt.
    let prompts = !spec.quiet;
    let record = spec.record.or_else(|| match (spec.cmds.as_slice(), &spec.cwd) {
        ([c], None) => UndoKind::classify(c),
        _ => None,
    });
    let (cmds, write_file, expect) = (spec.cmds, spec.write_file, spec.expect);
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
    let job = RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo,
    };
    launch(st, job, None, effect, fx)
}

/// Makes a job labelled `label` the active one in the active tab's repository, running the
/// effect `effect` builds for that folder, unless another job is running (said in the log).
/// Unlike `begin_job`, the trust prompt never holds it: it is for a program the user chose in
/// their own settings (the merge tool), and its copies of the repository's files are made
/// without the repository's filters while the repository is not trusted, so nothing the
/// repository configures runs. It keeps the undo history: the tool edits (and at most stages)
/// a working file and moves no refs, like the conflict view's Save and Mark resolved.
pub(super) fn begin_tool_job(st: &mut AppState, label: &str, effect: impl FnOnce(PathBuf) -> Effect, fx: &mut Vec<Effect>) -> Started {
    if busy(st, false) {
        return Started::No;
    }
    let Some(cwd) = st.workdir() else { return Started::No };
    st.job = Some(RunningJob {
        label: label.into(),
        workdir: Some(cwd.clone()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
    });
    fx.push(effect(cwd));
    Started::Now
}

/// Makes a job labelled `label` that runs in `cwd` (outside the active tab's repository: a clone
/// or an init) the active one, running the effect `effect` builds for that folder; once it
/// succeeds, `then_open` opens in the active tab. The trust gate applies as for git jobs.
pub(super) fn start_effect_job(
    st: &mut AppState,
    label: &str,
    cwd: PathBuf,
    then_open: Option<(PathBuf, Created)>,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    let job = RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open,
        keeps_undo: false,
    };
    launch(st, job, Some(cwd), effect, fx)
}

/// Ends a job that makes a repository (a sparse clone, an init): `made` says the repository
/// exists, `ok` that every step after it worked too. A repository made by a job that failed
/// later is still opened (and trusted as `then_open` says), under the failure's `headline`.
pub(super) fn finish_made(
    st: &mut AppState,
    made: bool,
    ok: bool,
    output: String,
    headline: Option<String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let open = if made && !ok { st.job.as_mut().and_then(|j| j.then_open.take()) } else { None };
    super::ops::finish_job(st, ok, output.clone(), headline, None, now, fx);
    if let Some((p, how)) = open {
        super::trust::created(st, &p, how, &output);
        super::tabs::open(st, p, fx);
    }
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
