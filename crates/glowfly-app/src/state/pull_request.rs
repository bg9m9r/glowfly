//! The pull request panel (update/pull_request/, ui/pull_request/): a copy of the pull
//! request, the remote-tracking tips it is compared from, the files it changes, the service's
//! page with its conversation, and the texts that describe them. Pure: the files and the page
//! are loaded by `Effect::PullRequest`; where the files and commits come from (and the notes on
//! it) is in pull_request_source.rs, the Commits tab's texts in pull_request_commits.rs, the
//! action forms' rules in pull_request_actions.rs, the code comment threads' in
//! pull_request_threads.rs (placed on the local version's lines by pull_request_line_map.rs
//! when the files aren't of the service's head), a new code comment on diff lines in pull_request_line_comment.rs,
//! and the Checks section's in pull_request_checks.rs.

use super::ForgeState;
use super::pull_request_actions::PrForms;
use super::pull_request_checks::PrChecks;
use super::pull_request_line_comment::LineDraft;
use super::pull_request_line_map::LineMaps;
use super::pull_request_source::{FilesKey, HeadFetch, Shown, files_key};
use super::pull_request_threads::ThreadView;
use git2::Oid;
use glowfly_core::forge::{PrPage, PrState, PullRequest, reviews_in_app};
use glowfly_core::repo::{BranchChanges, DiffTarget, FileEntry, RangeCommits, Snapshot, SourceTip};
use glowfly_core::text::is_bidi_control;

/// The local tips of a pull request's branches: `refs/remotes/<remote>/<target>` and the source
/// on the remote of the repository it is on (`ForgeState::source_remote`), when they exist.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tips {
    pub target: Option<Oid>,
    pub source: Option<Oid>,
}

impl Tips {
    pub fn of(snap: &Snapshot, remote: &str, source_remote: Option<&str>, pr: &PullRequest) -> Tips {
        Tips::in_remotes(&snap.remotes, remote, source_remote, pr)
    }

    /// `of`, from a snapshot's `remotes` (branch names there have no remote prefix, so a
    /// remote named `up/stream` never lends its branches to `up`).
    pub fn in_remotes(remotes: &[(String, Vec<(String, Oid)>)], remote: &str, source_remote: Option<&str>, pr: &PullRequest) -> Tips {
        let tip = |remote: &str, name: &str| {
            let branches = remotes.iter().find(|(r, _)| r == remote).map(|(_, b)| b.as_slice()).unwrap_or_default();
            branches.iter().find(|(b, _)| b == name).map(|(_, oid)| *oid)
        };
        Tips { target: tip(remote, &pr.target), source: source_remote.and_then(|r| tip(r, &pr.source)) }
    }
}

/// The files the pull request changes (the Commits tab follows the same load).
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
    Commits,
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
    /// Posting the new code comment on diff lines (`PrView::line_draft`).
    LineComment,
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
    /// The remote of the repository the source branch is on (`ForgeState::source_remote`):
    /// `remote`, a fork's, or None when that repository isn't a remote here.
    pub source_remote: Option<String>,
    /// Whether the last refresh of the service's list still had it.
    pub listed: bool,
    pub tips: Tips,
    pub files: Files,
    /// The service's head ref for it (`Provider::pr_head_ref`), when it keeps one.
    pub head_ref: Option<String>,
    /// The source tip and commits the loaded (or unrelated) files were compared to.
    pub shown: Option<Shown>,
    pub head_fetch: HeadFetch,
    /// The key the files were last asked for.
    pub asked: Option<FilesKey>,
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
    /// The unposted code comment on lines of a file's diff, if any.
    pub line_draft: Option<LineDraft>,
    /// The threads' lines mapped onto the local source tip, when it isn't the service's head.
    pub lines: LineMaps,
}

impl PrView {
    pub fn new(pr: PullRequest, fs: &ForgeState, snap: &Snapshot) -> PrView {
        let source_remote = fs.source_remote(&pr);
        let tips = Tips::of(snap, &fs.remote, source_remote.as_deref(), &pr);
        let page = initial_page(fs);
        PrView {
            label: fs.pr_label(pr.id),
            noun: fs.pr_noun(),
            remote: fs.remote.clone(),
            source_remote,
            listed: true,
            tips,
            files: Files::Missing,
            head_ref: fs.host_provider().and_then(|p| p.pr_head_ref(pr.id)),
            shown: None,
            head_fetch: HeadFetch::Idle,
            asked: None,
            pr,
            tab: PrTab::Overview,
            page,
            reloading: false,
            busy: None,
            action_error: None,
            forms: PrForms::default(),
            threads: ThreadView::default(),
            checks: PrChecks::new(fs),
            line_draft: None,
            lines: LineMaps::default(),
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

    /// The source tip the files were compared to, while they are loaded or unrelated.
    pub fn source_tip(&self) -> Option<Oid> {
        matches!(self.files, Files::Loaded(_) | Files::Unrelated).then(|| self.shown.as_ref().map(|s| s.source.oid)).flatten()
    }

    /// Whether `oid` is one of the commits the Commits tab lists.
    pub fn has_commit(&self, oid: Oid) -> bool {
        self.shown.as_ref().is_some_and(|s| s.commits.commits.iter().any(|c| c.oid == oid))
    }

    /// The files and commits are loaded, compared to `source`.
    pub fn set_loaded(&mut self, changes: BranchChanges, source: SourceTip, commits: RangeCommits) {
        self.files = Files::Loaded(changes);
        self.shown = Some(Shown { source, commits });
    }

    /// The key the panel compares from now.
    pub fn files_key(&self) -> FilesKey {
        files_key(self)
    }

    /// Whether `t` is the diff of one of this view's files or of one of its commits.
    pub fn shows(&self, t: &DiffTarget) -> bool {
        match t {
            DiffTarget::Commit(o, _) => self.has_commit(*o),
            _ => self.shows_file(t),
        }
    }

    /// Whether `t` is the diff of one of the files Files changed lists.
    pub fn shows_file(&self, t: &DiffTarget) -> bool {
        match (t, &self.files) {
            (DiffTarget::Between(a, b, _), Files::Loaded(c)) => c.base == *a && self.source_tip() == Some(*b),
            _ => false,
        }
    }

    /// The loaded files in list order, when (`a`, `b`) is (merge base, source tip).
    pub fn diff_files(&self, a: Oid, b: Oid) -> Option<&[FileEntry]> {
        let Files::Loaded(c) = &self.files else { return None };
        (c.base == a && self.source_tip() == Some(b)).then_some(c.comparison.files.as_slice())
    }

    /// The diff of `path` from the merge base to the source tip, once the files are loaded
    /// (the target its row opens).
    pub fn diff_target(&self, path: &str) -> Option<DiffTarget> {
        let Files::Loaded(c) = &self.files else { return None };
        Some(DiffTarget::Between(c.base, self.source_tip()?, path.to_owned()))
    }
}

/// The page a newly opened panel starts with: loading, or none for a service without in-app
/// reviews.
pub fn initial_page(fs: &ForgeState) -> Page {
    if fs.host_provider().is_some_and(reviews_in_app) { Page::Loading } else { Page::Unsupported }
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

#[cfg(test)]
pub(super) mod tests;
