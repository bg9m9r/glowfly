//! Messages and effects of the pull request panel (state/pull_request.rs,
//! state/pull_request_actions.rs, update/pull_request/mod.rs, update/pull_request/calls.rs,
//! runner/pull_request.rs, ui/pull_request/). Its files open through `RepoMsg::OpenDiff`, its
//! Fetch button sends `RemoteMsg::Fetch`, and the browser and copy actions are `Msg::OpenUrl`
//! and `Msg::Copy`.

use crate::effect::ForgeError;
use crate::state::pull_request::PrTab;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::forge::{Auth, Entry, MergeMethod, MergeRequest, Merged, PrPage, ReviewEvent, Target};
use trellis_core::repo::BranchChanges;

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
                PrCall::Comment(_) => "post-pr-comment",
                PrCall::Review { .. } => "submit-pr-review",
                PrCall::Merge { .. } => "merge-pr",
            },
        }
    }
}

/// What a service call answered.
#[derive(Clone, Debug, PartialEq)]
pub enum PrReply {
    Loaded(Result<PrPage, ForgeError>),
    Commented(Result<Entry, ForgeError>),
    Reviewed(ReviewEvent, Result<(), ForgeError>),
    Merged { method: MergeMethod, remote: String, source: String, delete: bool, result: Result<Merged, ForgeError> },
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
    /// Post the comment box's text.
    PostComment,
    /// Submit the review form.
    SubmitReview,
    /// Merge as the merge form says (after the confirmation).
    Merge,
    /// A service call's answer, for pull request `id` of `target` in `workdir`.
    Replied { workdir: PathBuf, target: Target, id: u64, generation: u64, reply: Box<PrReply> },
}

impl PullRequestMsg {
    /// Nothing is kept for an inactive tab: intents are for the active tab; a dropped
    /// FilesLoaded is asked for again by `reload_pending` when its tab becomes active; and
    /// Replied comes from an unscoped effect, so it never arrives scoped (its handler matches
    /// the repository itself and logs results for other tabs).
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(_)
            | Self::Close
            | Self::FilesLoaded { .. }
            | Self::Tab(_)
            | Self::Reload
            | Self::PostComment
            | Self::SubmitReview
            | Self::Merge
            | Self::Replied { .. } => None,
        }
    }
}
