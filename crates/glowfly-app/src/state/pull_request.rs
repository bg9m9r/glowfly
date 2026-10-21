//! The pull request panel (update/pull_request/, ui/pull_request/): a copy of the pull
//! request, the remote-tracking tips it is compared from, the files it changes, the service's
//! page with its conversation, and the texts that describe them. Pure: the files and the page
//! are loaded by `Effect::PullRequest`; the action forms' rules are in pull_request_actions.rs,
//! the code comment threads' in pull_request_threads.rs, and the Checks section's in
//! pull_request_checks.rs.

use super::ForgeState;
use super::pull_request_actions::PrForms;
use super::pull_request_checks::PrChecks;
use super::pull_request_threads::ThreadView;
use git2::Oid;
use glowfly_core::forge::{PrPage, PrState, PullRequest, reviews_in_app};
use glowfly_core::repo::{BranchChanges, DiffTarget, FileEntry, Snapshot};
use glowfly_core::text::is_bidi_control;

/// The local tips of a pull request's branches: `refs/remotes/<remote>/<target>` and
/// `refs/remotes/<remote>/<source>`, when they exist.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tips {
    pub target: Option<Oid>,
    pub source: Option<Oid>,
}

impl Tips {
    pub fn of(snap: &Snapshot, remote: &str, pr: &PullRequest) -> Tips {
        Tips::in_remotes(&snap.remotes, remote, pr)
    }

    /// `of`, from a snapshot's `remotes` (branch names there have no remote prefix, so a
    /// remote named `up/stream` never lends its branches to `up`).
    pub fn in_remotes(remotes: &[(String, Vec<(String, Oid)>)], remote: &str, pr: &PullRequest) -> Tips {
        let branches = remotes.iter().find(|(name, _)| name == remote).map(|(_, b)| b.as_slice()).unwrap_or_default();
        let tip = |name: &str| branches.iter().find(|(b, _)| b == name).map(|(_, oid)| *oid);
        Tips { target: tip(&pr.target), source: tip(&pr.source) }
    }

    /// (target, source) when both are present.
    pub fn pair(&self) -> Option<(Oid, Oid)> {
        Some((self.target?, self.source?))
    }
}

/// The files the pull request changes.
#[derive(Clone, PartialEq, Debug)]
pub enum Files {
    /// A tip is missing: nothing to compare (see `out_of_sync`).
    Missing,
    Loading,
    /// The two tips have no merge base.
    Unrelated,
    Loaded(BranchChanges),
    Failed(String),
}

/// The panel's tabs.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum PrTab {
    #[default]
    Overview,
    Files,
}

/// The service's page for the pull request.
#[derive(Clone, PartialEq, Debug)]
pub enum Page {
    /// The service has no conversation, reviews or merging in Glowfly.
    Unsupported,
    /// The repository isn't trusted, so nothing is asked.
    Held,
    Loading,
    Loaded(Box<PrPage>),
    Failed(String),
}

/// An action sent to the service; one runs at a time.
#[derive(Clone, PartialEq, Debug)]
pub enum PrAction {
    Comment,
    Review,
    Merge,
    /// A reply to the code comment thread with this id.
    Reply(String),
    /// Resolving or reopening the code comment thread with this id.
    Resolve(String),
}

/// The open pull request panel.
#[derive(Clone, Debug)]
pub struct PrView {
    /// The pull request as last listed by the service.
    pub pr: PullRequest,
    /// "#7" or "!7".
    pub label: String,
    /// "pull request" or "merge request".
    pub noun: &'static str,
    /// The remote that points at the service.
    pub remote: String,
    /// Whether the last refresh of the service's list still had it.
    pub listed: bool,
    pub tips: Tips,
    pub files: Files,
    pub tab: PrTab,
    pub page: Page,
    /// The page is loading again while the last one stays on screen.
    pub reloading: bool,
    pub busy: Option<PrAction>,
    /// The last action's failure, shown under its button.
    pub action_error: Option<(PrAction, String)>,
    pub forms: PrForms,
    /// Which code comment threads are shown and open.
    pub threads: ThreadView,
    /// Every check on its head commit.
    pub checks: PrChecks,
}

impl PrView {
    pub fn new(pr: PullRequest, fs: &ForgeState, snap: &Snapshot) -> PrView {
        let tips = Tips::of(snap, &fs.remote, &pr);
        let page = initial_page(fs);
        PrView {
            label: fs.pr_label(pr.id),
            noun: fs.pr_noun(),
            remote: fs.remote.clone(),
            listed: true,
            tips,
            files: Files::Missing,
            pr,
            tab: PrTab::Overview,
            page,
            reloading: false,
            busy: None,
            action_error: None,
            forms: PrForms::default(),
            threads: ThreadView::default(),
            checks: PrChecks::new(fs),
        }
    }

    /// The loaded page, if any.
    pub fn loaded(&self) -> Option<&PrPage> {
        match &self.page {
            Page::Loaded(p) => Some(p),
            _ => None,
        }
    }

    /// The service's state for it, once the page is loaded.
    pub fn state(&self) -> Option<PrState> {
        self.loaded().map(|p| p.detail.state)
    }

    /// Whether `t` is the diff of one of this view's files.
    pub fn shows(&self, t: &DiffTarget) -> bool {
        match (t, &self.files) {
            (DiffTarget::Between(a, b, _), Files::Loaded(c)) => c.base == *a && self.tips.source == Some(*b),
            _ => false,
        }
    }

    /// The loaded files in list order, when (`a`, `b`) is (merge base, source tip).
    pub fn diff_files(&self, a: Oid, b: Oid) -> Option<&[FileEntry]> {
        let Files::Loaded(c) = &self.files else { return None };
        (c.base == a && self.tips.source == Some(b)).then_some(c.comparison.files.as_slice())
    }

    /// The diff of `path` from the merge base to the source tip, once the files are loaded
    /// (the target its row opens).
    pub fn diff_target(&self, path: &str) -> Option<DiffTarget> {
        let Files::Loaded(c) = &self.files else { return None };
        Some(DiffTarget::Between(c.base, self.tips.source?, path.to_owned()))
    }
}

/// The page a newly opened panel starts with: loading, or none for a service without in-app
/// reviews.
pub fn initial_page(fs: &ForgeState) -> Page {
    if fs.host_provider().is_some_and(reviews_in_app) { Page::Loading } else { Page::Unsupported }
}

/// Why the local branches may not show the pull request as the service has it.
#[derive(Clone, PartialEq, Debug)]
pub enum OutOfSync {
    TargetMissing,
    SourceMissing,
    /// The source branch is gone because it was deleted once merged.
    SourceDeleted,
    /// The local source tip is not the head the service reported.
    SourceDiffers {
        local: Oid,
        head: String,
    },
}

impl OutOfSync {
    pub fn text(&self, v: &PrView) -> String {
        let (r, pr) = (&v.remote, &v.pr);
        match self {
            OutOfSync::TargetMissing => format!("{r}/{} isn't in this repository. Fetch {r} to compare.", pr.target),
            OutOfSync::SourceMissing => {
                format!("{r}/{} isn't in this repository. Fetch {r} to compare. A branch on a fork can't be fetched from {r}.", pr.source)
            }
            OutOfSync::SourceDeleted => format!("{r}/{} was deleted after the merge.", pr.source),
            OutOfSync::SourceDiffers { local, head } => format!(
                "{r}/{} is at {}, but the {} is at {}. Fetch {r} to see its latest changes.",
                pr.source,
                short(&local.to_string()),
                v.noun,
                short(head)
            ),
        }
    }
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// The notes to show above the file list, one per problem.
pub fn out_of_sync(v: &PrView) -> Vec<OutOfSync> {
    let mut notes = Vec::new();
    if v.tips.target.is_none() {
        notes.push(OutOfSync::TargetMissing);
    }
    match (v.tips.source, &v.pr.head) {
        (None, _) if v.state() == Some(PrState::Merged) => notes.push(OutOfSync::SourceDeleted),
        (None, _) => notes.push(OutOfSync::SourceMissing),
        (Some(local), Some(head)) if !v.pr.head_matches(&local.to_string()) => {
            notes.push(OutOfSync::SourceDiffers { local, head: head.clone() })
        }
        _ => {}
    }
    notes
}

pub fn files_heading(n: usize) -> String {
    if n == 1 { "1 file changed".to_owned() } else { format!("{n} files changed") }
}

pub fn review_summary(pr: &PullRequest) -> String {
    match pr.reviewers.len() {
        0 => "No reviewers".to_owned(),
        n => format!("{} of {n} approved", pr.approvals()),
    }
}

/// How a reviewer's state is colored.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Good,
    Bad,
    /// Needs attention, but isn't a failure (amber).
    Warn,
    Neutral,
}

pub fn reviewer_tone(state: &str) -> Tone {
    let s = state.to_lowercase();
    if s.starts_with("approved") {
        Tone::Good
    } else if s.contains("changes") || s.contains("rejected") {
        Tone::Bad
    } else {
        Tone::Neutral
    }
}

/// The note shown when the two tips have no merge base.
pub fn unrelated_text(v: &PrView, shallow: bool) -> String {
    let text = format!("{} and {} have no commit in common here, so there is nothing to compare.", v.pr.target, v.pr.source);
    if shallow { format!("{text} This is a shallow clone: fetching more history may find it.") } else { text }
}

/// The note shown when the source brings no file changes.
pub fn empty_text(v: &PrView) -> String {
    format!("No files differ: {} has nothing that {} doesn't.", v.pr.source, v.pr.target)
}

/// The service's description, safe to show: bidi and control characters removed, except
/// newlines and tabs.
pub fn description_text(s: &str) -> String {
    s.chars().filter(|&c| !is_bidi_control(c) && (!c.is_control() || c == '\n' || c == '\t')).collect()
}

/// Why the panel's Fetch button is disabled.
pub fn fetch_blocked(snap: &Snapshot, remote: &str, idle: bool) -> Option<String> {
    match super::remotes::find(snap, remote) {
        None => Some(format!("{remote} is no longer a remote")),
        Some(info) => super::remotes::fetch_blocked(info, idle).map(str::to_owned),
    }
}

#[cfg(test)]
mod tests;
