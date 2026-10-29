//! A git command sequence to run as the active job (`update/jobs.rs::start_job`), what its last
//! command takes when it is a cherry-pick or revert, and the `Retry` a failed one's Try again
//! sends (`JobMsg::Retry`, update/job_retry.rs): the same spec once more, for the same tab.

use crate::state::auto_stash::AutoStashJob;
use crate::state::push_pr::PrStart;
use crate::state::tabs::TabId;
use crate::state::trust::Created;
use glowfly_core::journal::Op;
use glowfly_core::pick_ops::Picking;
use glowfly_core::undo::UndoKind;
use std::path::PathBuf;

/// A git command sequence to run as the (single) active job.
#[derive(Clone, Debug)]
pub struct JobSpec {
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
    pub write_file: Option<(PathBuf, Vec<u8>)>,
    /// Record the job for undo when it is this kind of operation. Left `None`, a single command
    /// run in the active tab's repository is recognised by `UndoKind::classify`.
    pub record: Option<UndoKind>,
    /// The journal note to write when the job moves HEAD or the op's branch. Left `None`, a job
    /// in the active tab's repository gets the note its recorded kind calls for.
    pub journal: Option<Op>,
    /// Run nothing unless this revision still resolves to this commit.
    pub expect: Option<(String, git2::Oid)>,
    /// The job starts with this auto-stash, to put back once it ends (update/auto_stash.rs).
    pub auto_stash: Option<AutoStashJob>,
    /// The job's last command is a cherry-pick or revert of these commits: it runs as
    /// Effect::Pick, carried past the commits that change nothing.
    pub pick: Option<PickPlan>,
    /// The check to run when the job fails (`Effect::RunGit::on_fail_check`).
    pub on_fail_check: Option<Vec<String>>,
}

/// What a cherry-pick or revert job's last command takes (`JobSpec::pick`).
#[derive(Clone, Debug)]
pub struct PickPlan {
    pub picking: Picking,
    /// The commits, in the command's order.
    pub oids: Vec<git2::Oid>,
    /// git has `--skip` (2.23 or newer).
    pub can_skip: bool,
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
            journal: None,
            expect: None,
            auto_stash: None,
            pick: None,
            on_fail_check: None,
        }
    }
}

/// A failed job to run once more (Try again): its spec as it was started, for the tab it was
/// started in. Refused once that tab is closed, or while it isn't the active one.
#[derive(Clone, Debug)]
pub struct Retry {
    pub tab: TabId,
    /// The repository that tab showed (the job may have run in another, `JobSpec::cwd`).
    pub workdir: Option<PathBuf>,
    pub spec: JobSpec,
}
