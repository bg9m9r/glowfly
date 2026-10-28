//! Running a custom command: found again by its file and name, its placeholders filled in for
//! where it runs from (a command that needs what isn't there is logged and not run), asked
//! about first when it says so, then started as the active job: a git command through
//! `start_job` (the trust gate, undo record, journal note and refresh of a toolbar job), a
//! shell command as `CustomCommandEffect::RunShell` through `begin_job`. Both end as
//! `JobMsg::JobDone`. The repository's own commands run only in a Trusted repository whose print
//! covers the commands file they were loaded from: in any other they are first confirmed with
//! their source file and script shown, and then, like in a trusted one whose file changed since,
//! they wait in the trust prompt here (`trust::hold`), which lists the file.

use super::jobs::{JobSpec, begin_job, busy, start_job};
use crate::custom_commands::{CommandRef, CustomCommandEffect, CustomCommandMsg, ShellRun, Target};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::custom_commands::{missing_text, vars_for};
use crate::state::trust::{Standing, TrustPrompt, prompt_for};
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
    let selected = match st.session.selection {
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
    // A repository's own command runs only while the repository is trusted with exactly the
    // commands file it was read from; else it waits in the trust prompt, which lists that file.
    if r.source == Source::Repo && !repo_commands_trusted(st) {
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
        let Some(prompt) = repo_commands_prompt(st) else { return };
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

/// Whether the active repository is Trusted with a print that covers the repository commands
/// as loaded (`CustomCommands::repo_digest`). An edit to the commands file made outside Glowfly
/// is loaded before the full load that would ask about it: those commands aren't covered.
fn repo_commands_trusted(st: &AppState) -> bool {
    let Some(snap) = st.snap.as_ref() else { return false };
    let print = st.saved_trust_for(&snap.real_workdir).or_else(|| st.saved_trust_for(&snap.workdir)).and_then(|t| t.print);
    let digest = st.custom_commands.repo_digest.as_deref();
    st.trust_standing() == Some(Standing::Trusted) && print.zip(digest).is_some_and(|(p, d)| p.covers_commands(d))
}

/// The trust prompt for running the repository's own commands: the active repository's, with
/// the commands file as loaded (what Trust and run trusts and runs), listed as gained when the
/// repository was trusted before without it.
fn repo_commands_prompt(st: &AppState) -> Option<TrustPrompt> {
    let snap = st.snap.as_ref()?;
    let mut prompt = prompt_for(&st.settings, snap);
    prompt.scan.commands = st.custom_commands.repo_digest.clone();
    let saved = st.saved_trust_for(&snap.real_workdir).or_else(|| st.saved_trust_for(&snap.workdir));
    if let Some(print) = saved.filter(|t| t.is_trusted()).and_then(|t| t.print) {
        prompt.gained = Some(prompt.scan.gained_since(&print));
    }
    Some(prompt)
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
            (
                Effect::RunGit {
                    cwd,
                    cmds,
                    write_file: None,
                    record,
                    journal,
                    prompts: true,
                    expect: None,
                    stoppable: false,
                    on_fail_check: None,
                },
                keeps,
            )
        }
        Expanded::Shell(script) => (Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd, script })), false),
    }
}
