//! Messages and effects of the pull request panel (state/pull_request.rs,
//! state/pull_request_actions.rs, state/pull_request_threads.rs, update/pull_request/,
//! runner/pull_request.rs, ui/pull_request/). Its files open through `RepoMsg::OpenDiff`, its
//! Fetch button sends `RemoteMsg::Fetch`, and the browser and copy actions are `Msg::OpenUrl`
//! and `Msg::Copy`.

use crate::effect::ForgeError;
use crate::state::pull_request::PrTab;
use git2::Oid;
use glowfly_core::forge::{Auth, CheckList, Entry, MergeMethod, MergeRequest, Merged, PrPage, ReviewEvent, Target, ThreadComment};
use glowfly_core::repo::BranchChanges;
use std::path::PathBuf;

/// The files pull request `id` changes, from its local tips.
#[derive(Debug)]
pub struct FilesJob {
    pub workdir: PathBuf,
    pub id: u64,
    /// (target, source)
    pub tips: (Oid, Oid),
    pub generation: u64,
}

/// Where a service call goes, with this session's credential for it, if any; `Debug` hides
/// the credential (Auth's own `Debug`).
#[derive(Debug)]
pub struct PrRequest {
    pub workdir: PathBuf,
    pub target: Target,
    /// None signs in (environment, saved token, Git Credential Manager).
    pub auth: Option<Auth>,
    /// The user connected this account in Preferences.
    pub connected: bool,
}

/// What a service call does.
#[derive(Debug)]
pub enum PrCall {
    /// The pull request's page: detail and conversation.
    Load,
    /// Every check on its head commit (`head`: the listed head, when known).
    Checks {
        head: Option<String>,
    },
    Comment(String),
    Review {
        event: ReviewEvent,
        body: String,
        pending: Option<u64>,
    },
    /// Merge; once merged, `source` is deleted on `remote` (when `delete`) and `remote` fetched.
    Merge {
        req: MergeRequest,
        remote: String,
        source: String,
        delete: bool,
    },
    /// Reply to code comment thread `thread`, whose first comment is `comment`.
    Reply {
        thread: String,
        comment: u64,
        body: String,
    },
    /// Resolve (`resolve`) or reopen code comment thread `thread`.
    Resolve {
        thread: String,
        resolve: bool,
    },
}

#[derive(Debug)]
pub struct CallJob {
    pub req: PrRequest,
    pub id: u64,
    pub generation: u64,
    pub call: PrCall,
}

#[derive(Debug)]
pub enum PrEffect {
    /// The file list, from local refs (scoped to its repository; reads only).
    Files(FilesJob),
    /// A call to the hosting service (unscoped: its reply names its repository).
    Call(Box<CallJob>),
}

impl PrEffect {
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            PrEffect::Files(j) => Some(&j.workdir),
            PrEffect::Call(_) => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PrEffect::Files(_) => "load-pr-files",
            PrEffect::Call(j) => match j.call {
                PrCall::Load => "load-pr-page",
                PrCall::Checks { .. } => "load-pr-checks",
                PrCall::Comment(_) => "post-pr-comment",
                PrCall::Review { .. } => "submit-pr-review",
                PrCall::Merge { .. } => "merge-pr",
                PrCall::Reply { .. } => "reply-pr-thread",
                PrCall::Resolve { .. } => "resolve-pr-thread",
            },
        }
    }
}

/// What a service call answered.
#[derive(Clone, Debug, PartialEq)]
pub enum PrReply {
    Loaded(Result<PrPage, ForgeError>),
    Checks(Result<CheckList, ForgeError>),
    Commented(Result<Entry, ForgeError>),
    Reviewed(ReviewEvent, Result<(), ForgeError>),
    Merged {
        method: MergeMethod,
        remote: String,
        source: String,
        delete: bool,
        result: Result<Merged, ForgeError>,
    },
    RepliedToThread {
        thread: String,
        result: Result<ThreadComment, ForgeError>,
    },
    /// The thread's resolved state now.
    Resolved {
        thread: String,
        resolve: bool,
        result: Result<bool, ForgeError>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PullRequestMsg {
    /// Open the panel for pull request .0 (the sidebar row's double-click and menu).
    Open(u64),
    /// Close the panel (its Close button, Esc).
    Close,
    /// The file list of pull request `id`, compared from `tips` (target, source).
    FilesLoaded { id: u64, tips: (Oid, Oid), generation: u64, result: Result<Option<BranchChanges>, String> },
    /// Show Overview or Files changed.
    Tab(PrTab),
    /// Load the page from the service again (Try again).
    Reload,
    /// Compare the branches again after Files changed failed (Try again).
    ReloadFiles,
    /// Open or close the Checks section (its header).
    ToggleChecks,
    /// Load the checks again (the Checks section's Try again).
    ReloadChecks,
    /// Post the comment box's text.
    PostComment,
    /// Submit the review form.
    SubmitReview,
    /// Merge as the merge form says (after the confirmation).
    Merge,
    /// Post code comment thread .0's reply box.
    Reply(String),
    /// Resolve (`resolve`) or reopen code comment thread `thread`.
    Resolve { thread: String, resolve: bool },
    /// Show or hide resolved threads.
    ShowResolved(bool),
    /// Show or hide outdated threads.
    ShowOutdated(bool),
    /// A thread card's Show/Hide.
    ToggleThread(String),
    /// A diff line's comment marker was clicked: open thread .0 and scroll the pane to it.
    FocusThread(String),
    /// Open thread .0's file in Files changed, at its line.
    GoToThread(String),
    /// A service call's answer, for pull request `id` of `target` in `workdir`.
    Replied { workdir: PathBuf, target: Target, id: u64, generation: u64, reply: Box<PrReply> },
}

impl PullRequestMsg {
    /// Nothing is kept for an inactive tab: intents (the thread and Checks ones too) are for the
    /// active tab; a dropped FilesLoaded is asked for again by `reload_pending` when its tab
    /// becomes active (as are the checks); and
    /// Replied comes from an unscoped effect, so it never arrives scoped (its handler matches
    /// the repository itself and logs results for other tabs).
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(_)
            | Self::Close
            | Self::FilesLoaded { .. }
            | Self::Tab(_)
            | Self::Reload
            | Self::ReloadFiles
            | Self::ToggleChecks
            | Self::ReloadChecks
            | Self::PostComment
            | Self::SubmitReview
            | Self::Merge
            | Self::Reply(_)
            | Self::Resolve { .. }
            | Self::ShowResolved(_)
            | Self::ShowOutdated(_)
            | Self::ToggleThread(_)
            | Self::FocusThread(_)
            | Self::GoToThread(_)
            | Self::Replied { .. } => None,
        }
    }
}
