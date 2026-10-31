//! Cancelling the active job: whether it can be stopped now (the command it runs is a network
//! one: a fetch, a push, a clone, or a pull's fetch half), whether a stop was asked for, whether
//! the runner reported it stopped, and the transfer's progress as git last reported it.

use super::AppState;
use glowfly_core::git_progress::{Phase, Progress};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::size_text;

/// The kind of command a stoppable job is running (`JobMsg::Step`), which decides whether Cancel
/// can stop it now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    /// A fetch (also a pull's): stopping it leaves the repository as it was.
    #[default]
    Fetch,
    /// A push: the remote updates its branches only once it has the whole upload.
    Push,
    /// A clone: the folder it was making is removed.
    Clone,
    /// A pull's merge, or a pull run whole (it merges in the same git): never stopped part-way.
    Merge,
    /// A pull's rebase: never stopped part-way.
    Rebase,
    /// Any other command (a branch deleted before its remote one): it runs to its end, and a
    /// stop asked for meanwhile ends the job before the next command.
    Local,
}

impl Step {
    /// The step that runs `cmd` (its arguments, without `git`). A whole `git pull` merges in the
    /// same process as its fetch, so it is a merge.
    pub fn of(cmd: &[String]) -> Step {
        match glowfly_core::gitcli::subcommand(cmd) {
            Some("fetch") => Step::Fetch,
            Some("push") => Step::Push,
            Some("clone") => Step::Clone,
            Some("pull" | "merge") => Step::Merge,
            Some("rebase") => Step::Rebase,
            _ => Step::Local,
        }
    }

    /// Whether a job in this step can be stopped now.
    pub fn stoppable(self) -> bool {
        matches!(self, Step::Fetch | Step::Push | Step::Clone)
    }

    /// Whether a job made of `cmds` has a step a stop can end (one that goes to the network).
    pub fn any_network(cmds: &[Vec<String>]) -> bool {
        cmds.iter().any(|c| Step::of(c).stoppable() || is_pull(c))
    }
}

fn is_pull(cmd: &[String]) -> bool {
    glowfly_core::gitcli::subcommand(cmd) == Some("pull")
}

/// The active job's stop, set when it starts with a stoppable effect.
#[derive(Debug, Default)]
pub struct JobStop {
    /// Cancel was pressed; the runner was asked to stop it.
    pub requested: bool,
    /// The runner reported it stopped (`JobMsg::Stopped`): it ends as cancelled.
    pub stopped: bool,
    /// The command it is running now, as the runner last said (`JobMsg::Step`).
    pub step: Step,
    /// The transfer's progress as git last printed it (`JobMsg::Progress`), for this step.
    pub progress: Option<Progress>,
}

impl JobStop {
    /// The stop of a job whose first command is `first`: a pull starts with its fetch.
    pub fn starting(first: Option<&[String]>) -> Self {
        let step = match first {
            Some(c) if is_pull(c) => Step::Fetch,
            Some(c) => Step::of(c),
            None => Step::Fetch,
        };
        Self { step, ..Self::default() }
    }
}

impl AppState {
    /// A job runs that can be stopped now and hasn't been asked to yet (the status bar's Cancel).
    pub fn job_cancellable(&self) -> bool {
        self.job.is_some() && self.job_stop.as_ref().is_some_and(|s| !s.requested && s.step.stoppable())
    }

    /// A stoppable job runs a step that can't be stopped (a pull's merge): that step, for the
    /// disabled Cancel's reason.
    pub fn job_cancel_blocked(&self) -> Option<Step> {
        self.job.as_ref()?;
        self.job_stop.as_ref().filter(|s| !s.requested && !s.step.stoppable()).map(|s| s.step)
    }

    /// The running job was asked to stop and hasn't ended yet.
    pub fn job_cancelling(&self) -> bool {
        self.job.is_some() && self.job_stop.as_ref().is_some_and(|s| s.requested)
    }

    /// The running job's transfer progress, when git has reported one for its current step.
    pub fn job_progress(&self) -> Option<&Progress> {
        self.job.as_ref()?;
        self.job_stop.as_ref()?.progress.as_ref()
    }
}

/// `p` for the status bar, with git's numbers in Glowfly's own form: "Receiving 45% · 1.2 MB ·
/// 300.0 KB/s", "Counting 5".
pub fn progress_text(t: Lang, p: &Progress) -> String {
    let phase = if p.remote {
        t.tr("status.job.progress.remote")
    } else {
        match p.phase {
            Phase::Enumerating => t.tr("status.job.progress.enumerating"),
            Phase::Counting => t.tr("status.job.progress.counting"),
            Phase::Compressing => t.tr("status.job.progress.compressing"),
            Phase::Receiving => t.tr("status.job.progress.receiving"),
            Phase::Writing => t.tr("status.job.progress.sending"),
            Phase::Resolving => t.tr("status.job.progress.resolving"),
            Phase::Checkout => t.tr("status.job.progress.checkout"),
        }
    };
    let mut parts = vec![match p.percent {
        Some(percent) => t.trf("status.job.progress.percent", &[("phase", &phase), ("percent", &percent)]),
        None => t.trf("status.job.progress.count", &[("phase", &phase), ("count", &p.done)]),
    }];
    parts.extend(p.bytes.map(size_text));
    parts.extend(p.rate.filter(|&r| r > 0).map(|r| t.trf("status.job.progress.rate", &[("size", &size_text(r))])));
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::{JobStop, Step};
    use glowfly_core::gitcli::args;

    #[test]
    fn network_commands_can_be_stopped_and_merges_cannot() {
        assert_eq!(Step::of(&args(&["fetch", "origin"])), Step::Fetch);
        assert_eq!(Step::of(&args(&["-c", "a=b", "push", "origin", "main"])), Step::Push);
        assert_eq!(Step::of(&args(&["clone", "--", "u", "r"])), Step::Clone);
        assert_eq!(Step::of(&args(&["pull", "--ff"])), Step::Merge);
        assert_eq!(Step::of(&args(&["rebase", "--onto", "a", "b"])), Step::Rebase);
        assert_eq!(Step::of(&args(&["branch", "-d", "x"])), Step::Local);
        assert!(Step::Push.stoppable() && !Step::Merge.stoppable() && !Step::Local.stoppable());
        assert!(Step::any_network(&[args(&["branch", "-d", "x"]), args(&["push", "origin", "--delete", "x"])]));
        assert!(Step::any_network(&[args(&["pull"])]));
        assert!(!Step::any_network(&[args(&["tag", "v1"])]));
        assert_eq!(JobStop::starting(Some(&args(&["pull", "--ff"]))).step, Step::Fetch, "a pull starts by fetching");
        assert_eq!(JobStop::starting(Some(&args(&["branch", "-d", "x"]))).step, Step::Local);
    }

    #[test]
    fn progress_is_shown_in_glowflys_own_words_and_units() {
        let t = glowfly_core::i18n::Lang::En;
        let parse = |l| glowfly_core::git_progress::parse(l).unwrap();
        let text = |l| super::progress_text(t, &parse(l));
        assert_eq!(text("Receiving objects:  45% (450/1000), 1.20 MiB | 300.00 KiB/s"), "Receiving 45% · 1.2 MB · 300.0 KB/s");
        assert_eq!(text("Writing objects: 100% (3/3), 260 bytes | 0 bytes/s, done."), "Sending 100% · 260 bytes");
        assert_eq!(text("remote: Counting objects:  50% (5/10)"), "Server preparing 50%");
        assert_eq!(text("Enumerating objects: 5, done."), "Listing objects 5");
        assert_eq!(text("Updating files:  10% (1/10)"), "Checking out 10%");
    }
}
