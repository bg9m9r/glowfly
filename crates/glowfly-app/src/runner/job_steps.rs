//! Running a stoppable job's git commands (`Effect::RunGit` with `stoppable`, whose stop
//! `Effect::StopGit(StopKey::Job)` asks for). Before each command the UI is told its kind
//! (`JobMsg::Step`), so Cancel is offered only while one can be stopped:
//!
//! - A fetch, push or clone runs with `--progress` through `gitcli::output_watching`: a stop
//!   ends git's whole process tree (remote helpers and ssh too), and its progress lines are
//!   sent as `JobMsg::Progress`, a few times a second at most, and folded in the log.
//! - A pull runs as its fetch and then its merge or rebase (`trust::pull::run_split`), so only
//!   the fetch can be stopped. Whatever the split doesn't model (submodules, a fetch that may
//!   store into the branch checked out) and every way it would fail before its merge or rebase
//!   (divergent branches, no upstream, an upstream gone) run as `git pull` itself, which can't
//!   be stopped once it starts, so the outcome and git's words are `git pull`'s.
//! - Any other command, a merge or rebase included, runs to its end; a stop asked meanwhile
//!   ends the job before the next command starts. Nothing here kills a merge or rebase.
//!
//! After a stop the lock and temporary files a fetch or push left are removed
//! (`stop_cleanup`, scanned before the first one ran), and so is what a stopped clone made
//! (`clone_stop`), once its processes have exited.

use super::git_ops::job_env;
use crate::msg::{JobMsg, Msg};
use crate::state::job_stop::Step;
use glowfly_core::git_progress::{self, Phase, Progress, Tail};
use glowfly_core::gitcli::{self, Stop};
use glowfly_core::{clone_stop, stop_cleanup, undo};
use std::path::Path;
use std::time::{Duration, Instant};

/// The least time between two progress messages of the same phase.
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// How a stoppable job's commands went.
pub(super) struct Ran {
    pub ok: bool,
    pub output: String,
    /// A stop ended it (the output says so, and what was removed).
    pub stopped: bool,
    /// Where the operation of its last command started, when asked to note it (`op_stops`).
    pub start: Option<undo::OpStart>,
}

/// Runs `cmds` in `cwd` in order, stopping at the first failure or once `stop` is asked; with
/// `note`, where the last command's operation starts is noted just before it.
pub(super) fn run(cwd: &Path, cmds: &[Vec<String>], prompts: bool, stop: &Stop, send: &dyn Fn(Msg), note: bool) -> Ran {
    let mut live = Live::new(cwd, prompts, stop, send);
    let mut output = String::new();
    let mut start = None;
    let mut ok = true;
    for (at, c) in cmds.iter().enumerate() {
        // A stop asked during a command that runs to its end comes before the next one.
        if stop.requested() {
            live.stopped = true;
            break;
        }
        if note && at + 1 == cmds.len() {
            start = undo::op_start(cwd);
        }
        let (done, text) = live.command(c);
        if !text.is_empty() {
            output.push_str(&text);
            output.push('\n');
        }
        if live.stopped || !done {
            ok = false;
            break;
        }
    }
    let output = live.finish(output);
    Ran { ok: ok && !live.stopped, output, stopped: live.stopped, start }
}

/// One stoppable job's commands as they run.
pub(super) struct Live<'a> {
    cwd: &'a Path,
    prompts: bool,
    stop: &'a Stop,
    send: &'a dyn Fn(Msg),
    /// A stop ended a command, or came before one.
    pub stopped: bool,
    /// The kind of command last announced.
    step: Option<Step>,
    /// How many commands have started.
    started: usize,
    /// The repository's lock and temporary files before its first fetch or push.
    before: Option<stop_cleanup::Before>,
    /// The folder of the clone that was stopped.
    clone: Option<clone_stop::Target>,
}

impl<'a> Live<'a> {
    pub fn new(cwd: &'a Path, prompts: bool, stop: &'a Stop, send: &'a dyn Fn(Msg)) -> Self {
        Self { cwd, prompts, stop, send, stopped: false, step: None, started: 0, before: None, clone: None }
    }

    /// Runs the job command `c`, echoed as `$ git …` first: a splittable pull as its fetch and
    /// its merge or rebase, anything else as it is. Whether it succeeded, and the output.
    fn command(&mut self, c: &[String]) -> (bool, String) {
        let echo = format!("$ git {}", glowfly_core::text::redact_credentials(&c.join(" ")));
        if gitcli::subcommand(c) == Some("pull")
            && let Some(pull) = glowfly_core::trust::pull::Pull::of(c)
        {
            let (cwd, started) = (self.cwd, self.started);
            let run = glowfly_core::trust::pull::run_split(cwd, &pull, &mut |a, i, e| self.git(a, i, e));
            let (glowfly_core::trust::pull::PullRun::Ran { output, .. } | glowfly_core::trust::pull::PullRun::Refused { output, .. }) =
                &run;
            // A stop before any of its commands started says only that: a check it cut short (the
            // clean tree a rebase needs) didn't fail. After one started, what they did stays in the
            // log (a fetch that moved the branch, and what that left).
            if self.stopped {
                return (false, if self.started > started { output.clone() } else { echo });
            }
            return match run {
                glowfly_core::trust::pull::PullRun::Ran { ok, output } => (ok, output),
                // With nothing to compare, only an unreadable repository refuses a pull.
                glowfly_core::trust::pull::PullRun::Refused { output, .. } => {
                    (false, format!("{output}\nCould not read the repository to pull into.").trim_start().to_owned())
                }
            };
        }
        let (ok, text) = self.git(c, "", &[]);
        (ok, if text.is_empty() { echo } else { format!("{echo}\n{text}") })
    }

    /// Runs one git command (`trust::pull::Git`): a network one so a stop ends it, anything
    /// else to its end unless a stop came first.
    pub fn git(&mut self, cmd: &[String], input: &str, extra: &[(&str, &str)]) -> (bool, String) {
        if self.stop.requested() {
            self.stopped = true;
            return (false, format!("git {} was cancelled before it started", cmd.join(" ")));
        }
        let step = Step::of(cmd);
        if self.step != Some(step) {
            self.step = Some(step);
            (self.send)(Msg::Job(JobMsg::Step(step)));
        }
        self.started += 1;
        let env: Vec<(&str, &str)> = job_env(self.prompts).iter().chain(extra).copied().collect();
        if !step.stoppable() {
            return gitcli::run_with_input(self.cwd, cmd, input, &env);
        }
        let clone = if step == Step::Clone { clone_stop::note(self.cwd, cmd) } else { None };
        if step != Step::Clone && self.before.is_none() {
            self.before = stop_cleanup::scan(self.cwd);
        }
        let mut watch = Watch::default();
        let send = self.send;
        let out = gitcli::output_watching(self.cwd, &with_progress(cmd), input, &env, self.stop, &mut |chunk| {
            if let Some(p) = watch.chunk(chunk, Instant::now()) {
                send(Msg::Job(JobMsg::Progress(p)));
            }
        });
        if out.stopped {
            self.stopped = true;
            self.clone = clone;
        }
        (out.ok, git_progress::fold(&out.combined()))
    }

    /// `output` with what a stop removed added: the job's log once it ended.
    pub fn finish(&self, mut output: String) -> String {
        if !self.stopped {
            return output.trim_end().to_owned();
        }
        output.push_str("Cancelled.\n");
        let removed = self.before.as_ref().map(stop_cleanup::clean).unwrap_or_default();
        match removed.len() {
            0 => {}
            1 => output.push_str("Removed 1 file the stopped command left behind.\n"),
            n => output.push_str(&format!("Removed {n} files the stopped command left behind.\n")),
        }
        match self.clone.as_ref().map(clone_stop::clean) {
            Some(clone_stop::Cleaned::Folder(p)) => {
                output.push_str(&format!("Removed {}, which the stopped clone had made.\n", glowfly_core::native_path::shown(&p)))
            }
            Some(clone_stop::Cleaned::Entries(p, n)) => output.push_str(&format!(
                "Removed what the stopped clone had put in {} ({n} items); the folder itself is kept.\n",
                glowfly_core::native_path::shown(&p)
            )),
            Some(clone_stop::Cleaned::Failed(p, why)) => {
                output.push_str(&format!("Could not remove {}: {why}\n", glowfly_core::native_path::shown(&p)))
            }
            Some(clone_stop::Cleaned::Nothing) | None => {}
        }
        output.trim_end().to_owned()
    }
}

/// `cmd` with `--progress` after its subcommand, so git reports progress to a pipe too, unless
/// it already asks for progress or quiet.
fn with_progress(cmd: &[String]) -> Vec<String> {
    let set = |a: &String| matches!(a.as_str(), "--progress" | "--no-progress" | "-q" | "--quiet");
    let Some(at) = sub_index(cmd).filter(|_| !cmd.iter().any(set)) else { return cmd.to_vec() };
    let mut out = cmd.to_vec();
    out.insert(at + 1, "--progress".to_owned());
    out
}

/// The index of `cmd`'s subcommand, as `gitcli::subcommand` finds it.
fn sub_index(cmd: &[String]) -> Option<usize> {
    let mut i = 0;
    while i < cmd.len() {
        match cmd[i].as_str() {
            "-c" | "-C" => i += 2,
            s if s.starts_with('-') => i += 1,
            _ => return Some(i),
        }
    }
    None
}

/// The progress lines worth sending from a command's stderr: the newest one, when its phase
/// changed or [`PROGRESS_EVERY`] passed since the last one sent.
#[derive(Default)]
struct Watch {
    tail: Tail,
    sent: Option<(Phase, bool, Instant)>,
}

impl Watch {
    fn chunk(&mut self, chunk: &[u8], now: Instant) -> Option<Progress> {
        let p = self.tail.push(chunk)?;
        let due = match self.sent {
            Some((phase, remote, at)) => phase != p.phase || remote != p.remote || now.duration_since(at) >= PROGRESS_EVERY,
            None => true,
        };
        due.then(|| {
            self.sent = Some((p.phase, p.remote, now));
            p
        })
    }
}

#[cfg(test)]
#[path = "job_steps_tests.rs"]
mod tests;
