//! A tab's state at a glance (design pass D15): what its tab's state slot shows, the status
//! bar's last failure, and the tab's tooltip and accessible name. The counts (`Glance`) are the
//! active tab's snapshot's; a parked tab keeps its snapshot's, updated by the status reads that
//! still arrive for it (`CountsRead`, from `Msg::on_inactive`). The running job is the one whose
//! repository is the tab's, and the last failure is the newest run in the tab's activity log
//! when that run failed, until it is dismissed (`LogsMsg::DismissFailure`) or a later run
//! succeeds.

use crate::state::AppState;
use crate::state::logs::{EntryId, LogEntry};
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{FileEntry, Snapshot};

/// The counts a tab shows, from its last status read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glance {
    /// Changed paths, each counted once (`ChangeCounts::total`).
    pub changed: usize,
    pub conflicted: usize,
    /// The operation in progress; None when the repository is clean of one.
    pub op: Option<RepositoryState>,
    /// The checked-out branch against its upstream; 0 without one.
    pub ahead: usize,
    pub behind: usize,
    pub upstream: Option<String>,
}

impl Glance {
    pub fn of(snap: &Snapshot) -> Self {
        let head = snap.head_branch.as_ref().and_then(|_| snap.head_info()).filter(|b| b.upstream.is_some());
        Self {
            changed: snap.changes.total(),
            conflicted: conflicted(&snap.unstaged),
            op: op(snap.state),
            ahead: head.map_or(0, |b| b.ahead),
            behind: head.map_or(0, |b| b.behind),
            upstream: head.and_then(|b| b.upstream.clone()),
        }
    }

    /// Whether a merge, rebase or other operation is in progress, or conflicts are left.
    pub fn in_progress(&self) -> bool {
        self.op.is_some() || self.conflicted > 0
    }
}

fn op(state: RepositoryState) -> Option<RepositoryState> {
    (state != RepositoryState::Clean).then_some(state)
}

fn conflicted(unstaged: &[FileEntry]) -> usize {
    unstaged.iter().filter(|f| f.status == '!').count()
}

/// A read for a parked tab that moves its counts (`Inactive::Counts`).
#[derive(Debug, PartialEq)]
pub enum CountsRead {
    /// A whole load: every count.
    Load(Box<Glance>),
    /// A whole status read: the changes and the operation; the branch counts stay.
    Status { changed: usize, conflicted: usize, op: Option<RepositoryState> },
}

impl CountsRead {
    /// A whole status read's lists and state.
    pub fn status(unstaged: &[FileEntry], staged: &[FileEntry], state: RepositoryState) -> Self {
        let changed = glowfly_core::repo::ChangeCounts::of(staged, unstaged).total();
        Self::Status { changed, conflicted: conflicted(unstaged), op: op(state) }
    }

    pub fn apply(self, g: &mut Glance) {
        match self {
            Self::Load(new) => *g = *new,
            Self::Status { changed, conflicted, op } => (g.changed, g.conflicted, g.op) = (changed, conflicted, op),
        }
    }
}

/// The run a tab's last failure is: its title (the job's label) and log entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub label: String,
    pub entry: EntryId,
}

impl Failure {
    /// "Fetch failed": the job's label, with any fetch (all remotes, or one) said as Fetch.
    pub fn headline(&self, t: Lang) -> String {
        let job = if self.label.starts_with("Fetch") && !self.label.starts_with("Fetch and") {
            t.tr("job.label.fetch").to_owned()
        } else {
            crate::state::job_spec::labels::shown(t, &self.label).into_owned()
        };
        t.trf("glance.failed", &[("job", &job)])
    }
}

/// What one tab shows besides its name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabGlance {
    /// The running job's label, while it runs in this tab's repository.
    pub running: Option<String>,
    pub failed: Option<Failure>,
    /// None for a new tab, and a tab that hasn't loaded yet.
    pub counts: Option<Glance>,
}

/// The one state a tab's slot shows: the first that holds, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Running,
    Failed,
    InProgress,
    Changed,
    /// Behind its upstream by this many commits.
    Behind(usize),
}

impl TabGlance {
    pub fn mark(&self) -> Option<Mark> {
        let c = self.counts.as_ref();
        if self.running.is_some() {
            Some(Mark::Running)
        } else if self.failed.is_some() {
            Some(Mark::Failed)
        } else if c.is_some_and(Glance::in_progress) {
            Some(Mark::InProgress)
        } else if c.is_some_and(|c| c.changed > 0) {
            Some(Mark::Changed)
        } else {
            c.filter(|c| c.behind > 0).map(|c| Mark::Behind(c.behind))
        }
    }

    /// Every state, in the slot's order: "3 changed files, 2 behind origin/main".
    pub fn states(&self, t: Lang) -> Vec<String> {
        let label = |l: &str| crate::state::job_spec::labels::shown(t, l).into_owned();
        let mut out = Vec::new();
        if let Some(job) = &self.running {
            out.push(t.trf("glance.tab.running", &[("job", &label(job))]));
        }
        if let Some(f) = &self.failed {
            out.push(f.headline(t));
        }
        let Some(c) = &self.counts else { return out };
        if let Some(op) = c.op {
            out.push(op_name(t, op).to_owned());
        }
        if c.conflicted > 0 {
            out.push(t.trn("op_strip.conflicted", c.conflicted, &[]));
        }
        if c.changed > 0 {
            out.push(t.trn("glance.tab.changed", c.changed, &[]));
        }
        if let Some(up) = &c.upstream {
            if c.ahead > 0 {
                out.push(t.trn("glance.tab.ahead", c.ahead, &[("upstream", up)]));
            }
            if c.behind > 0 {
                out.push(t.trn("glance.tab.behind", c.behind, &[("upstream", up)]));
            }
        }
        out
    }

    /// The tab's tooltip line and accessible name: "api-server: 3 changed files, 2 behind
    /// origin/main", or only the name with nothing to say.
    pub fn describe(&self, t: Lang, name: &str) -> String {
        let states = self.states(t);
        if states.is_empty() {
            return name.to_owned();
        }
        t.trf("glance.tab.name", &[("name", &name), ("states", &states.join(", "))])
    }
}

/// The operation's name as the strip under the toolbar says it, without its step.
fn op_name(t: Lang, op: RepositoryState) -> &'static str {
    use RepositoryState::*;
    t.tr(match op {
        Clean => "op_strip.conflicts",
        Merge => "op_strip.merge",
        RebaseInteractive => "op_strip.interactive",
        Rebase | RebaseMerge => "op_strip.rebase",
        CherryPick | CherryPickSequence => "op_strip.cherry_pick",
        Revert | RevertSequence => "op_strip.revert",
        ApplyMailbox | ApplyMailboxOrRebase => "op_strip.am",
        Bisect => "op_strip.bisect",
    })
}

/// The last failure in `log`: its newest run of this session, when that run failed and its
/// failure wasn't dismissed.
fn last_failure(log: &[LogEntry], dismissed: &std::collections::HashSet<EntryId>) -> Option<Failure> {
    let run = log.iter().rev().find(|e| e.run.is_some() && e.earlier.is_none())?;
    let title = run.run.as_ref().map(|r| r.title.clone())?;
    (!run.ok && !dismissed.contains(&run.id)).then_some(Failure { label: title, entry: run.id })
}

impl AppState {
    /// What tab `i` shows besides its name.
    pub fn tab_glance(&self, i: usize) -> TabGlance {
        let Some(tab) = self.tabs.get(i) else { return TabGlance::default() };
        let active = i == self.active_tab;
        let running = self.job.as_ref().filter(|j| j.workdir.as_deref().is_some_and(|w| tab.is(w))).map(|j| j.label.clone());
        let log = if active { &self.log } else { &tab.log };
        let failed = tab.path.as_ref().and(last_failure(log, &self.logs.dismissed));
        let counts = if active {
            self.snap.as_ref().map(Glance::of)
        } else {
            tab.parked.as_deref().and_then(|p| p.glance.clone().or_else(|| p.snap.as_ref().map(Glance::of)))
        };
        TabGlance { running, failed, counts }
    }
}

#[cfg(test)]
#[path = "glance_tests.rs"]
mod tests;
