//! Running a custom command: found again by its file and name, its placeholders filled in for
//! where it runs from (a command that needs what isn't there is logged and not run), asked
//! about first when it says so, then started as the active job: a git command through
//! `start_job` (the trust gate, undo record, journal note and refresh of a toolbar job), a
//! shell command as `CustomCommandEffect::RunShell` through `begin_job`. Both end as
//! `JobMsg::JobDone`. The repository's own commands run only in a Trusted repository: in any
//! other they are first confirmed with their source file and script shown, then in one with no
//! decision and nothing the trust scan found they wait in the trust prompt here
//! (`trust::hold`), and in a held one the job gate holds them like any job.

use super::jobs::{JobSpec, begin_job, busy, start_job};
use crate::custom_commands::{CommandRef, CustomCommandEffect, CustomCommandMsg, ShellRun, Target};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::custom_commands::{missing_text, vars_for};
use crate::state::trust::{Standing, prompt_for};
use crate::state::{AppState, Modal, RunningJob, Selection};
use glowfly_core::custom_commands::{ConfirmText, Expanded, Source};
use glowfly_core::journal::Op;
use glowfly_core::undo::UndoKind;
use std::path::PathBuf;
use std::time::Instant;

pub(super) fn run(st: &mut AppState, r: CommandRef, at: Target, confirmed: bool, _now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = st.snap.as_ref() else { return };
    let workdir = snap.workdir.clone();
    let Some(cmd) = st.custom_commands.find(&r, Some(&workdir)).cloned() else {
        st.log(false, t.trf("commands.run.gone", &[("name", &r.name)]));
        return;
    };
    let selected = match st.selection {
        Selection::Commit(o) => Some(o),
        _ => None,
    };
    let open_file = st.session.diff.view.as_ref().map(|(d, _)| d.path().to_owned());
    let vars = vars_for(snap, &at, selected, open_file.as_deref());
    let v = vars.as_vars();
    let expanded = match cmd.expand(&v, cfg!(windows)) {
        Ok(e) => e,
        Err(m) => {
            st.log(false, t.trf("commands.run.missing", &[("name", &cmd.name), ("why", &missing_text(t, m))]));
            return;
        }
    };
    // A repository's own command in a repository the user hasn't trusted always asks first, and
    // says where it comes from and what it runs: the trust prompt that may follow speaks only of
    // the repository's settings and hooks.
    let from_repo = r.source == Source::Repo && st.trust_standing() != Some(Standing::Trusted);
    let question = cmd.confirm_text(&v).or(from_repo.then_some(ConfirmText::Default));
    if !confirmed && let Some(question) = question {
        let question = match question {
            ConfirmText::Default => t.trf("commands.confirm.default", &[("name", &cmd.name)]),
            ConfirmText::Text(q) => q,
        };
        let runs = t.trf("commands.confirm.runs", &[("command", &expanded.preview())]);
        let body = match from_repo {
            true => format!(
                "{question}

{}

{runs}",
                t.tr("commands.confirm.from_repo")
            ),
            false => format!(
                "{question}

{runs}"
            ),
        };
        let then = Box::new(Msg::CustomCommand(CustomCommandMsg::Run { command: r, at, confirmed: true }));
        let button = t.tr("commands.confirm.button").to_owned();
        st.modal = Some(Modal::Confirm { title: cmd.name.clone(), body, button, then });
        return;
    }
    if busy(st, false) {
        return;
    }
    // A repository's own command in one the trust scan found nothing in still asks: it runs code
    // the repository supplies. Held repositories hold every job at the gate anyway.
    let clean = st.trust_standing() == Some(Standing::Clean);
    if r.source == Source::Repo && clean {
        let (effect, keeps_undo) = job_effect(workdir.clone(), expanded);
        let job = RunningJob {
            label: cmd.name.clone(),
            workdir: Some(workdir),
            quiet: false,
            clear_commit: false,
            then_push: false,
            then_pr: None,
            then_link: false,
            then_open: None,
            keeps_undo,
            fetches: false,
        };
        let Some(snap) = st.snap.as_ref() else { return };
        let prompt = prompt_for(&st.settings, snap);
        super::trust::hold(st, job, effect, prompt, false);
        return;
    }
    match expanded {
        Expanded::Git(argv) => {
            start_job(st, JobSpec::new(cmd.name.clone(), vec![argv]), fx);
        }
        Expanded::Shell(script) => {
            begin_job(st, &cmd.name, false, |cwd| Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd, script })), fx);
        }
    }
}

/// The effect a held command runs once the repository is trusted, as `start_job` or
/// `begin_job` would have built it, and whether it keeps the undo history.
fn job_effect(cwd: PathBuf, e: Expanded) -> (Effect, bool) {
    match e {
        Expanded::Git(argv) => {
            let record = UndoKind::classify(&argv);
            let journal = record.as_ref().and_then(|k| Op::of_kind(k, &argv));
            let cmds = vec![argv];
            let keeps = super::undo::keeps_history(&cmds);
            (Effect::RunGit { cwd, cmds, write_file: None, record, journal, prompts: true, expect: None, stoppable: false }, keeps)
        }
        Expanded::Shell(script) => (Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd, script })), false),
    }
}
