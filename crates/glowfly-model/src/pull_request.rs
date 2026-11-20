//! Messages and effects of the pull request panel (state/pull_request.rs,
//! state/pull_request/source.rs, state/pull_request/commits.rs, state/pull_request/actions.rs,
//! state/pull_request/threads.rs, state/pull_request/line_map.rs, state/pull_request/line_comment.rs,
//! update/pull_request/,
//! runner/pull_request.rs, ui/pull_request/); a new code comment on diff lines has its own
//! messages in `line_comment`. Its files open through `RepoMsg::OpenDiff`, its
//! Fetch button sends `RemoteMsg::Fetch`, and the browser and copy actions are `Msg::OpenUrl`
//! and `Msg::Copy`.

use crate::effect::EffectMeta;
use crate::effect::SandboxRule;
pub mod line_comment;

use crate::effect::ForgeError;
use crate::state::pull_request::PrTab;
use git2::Oid;
use glowfly_core::forge::{
    Auth, CheckList, Entry, MergeMethod, MergeRequest, Merged, NewComment, NewThread, PrPage, ReviewEvent, Target, ThreadComment,
};
use glowfly_core::repo::{PrCompare, PrLineMaps};
use line_comment::LineCommentMsg;
use std::path::{Path, PathBuf};

/// The files and commits pull request `id` brings, compared from the target's
/// remote-tracking tip to a source tip the runner picks: the listed `head` when it is here,
/// else the remote-tracking source `branch`.
#[derive(Debug)]
pub struct FilesJob {
    pub workdir: PathBuf,
    pub id: u64,
    pub target: Oid,
    pub branch: Option<Oid>,
    /// The head the service listed (7 to 64 hex characters).
    pub head: Option<String>,
    pub generation: u64,
}

/// Fetches pull request `id`'s head ref into `refs/glowfly/pr/<id>` (`remote_ops::fetch_ref_cmd`):
/// `head` is the listed head it should bring. Without `prompts` it fails rather than asks for
/// credentials.
#[derive(Debug)]
pub struct HeadJob {
    pub workdir: PathBuf,
    pub id: u64,
    pub head: String,
    pub cmd: Vec<String>,
    pub prompts: bool,
}

/// A change to Glowfly's own refs under `refs/glowfly/pr/` that keep pull request heads
/// (`glowfly_core::pr_head_refs`). Best effort: a failure is only logged.
#[derive(Debug)]
pub enum HeadRefJob {
    /// Points `refs/glowfly/pr/<id>` at the listed head the files were compared to.
    Keep { workdir: PathBuf, id: u64, oid: Oid },
    /// Deletes the refs of the pull requests `ids`, which no panel copy shows.
    Drop { workdir: PathBuf, ids: Vec<u64> },
    /// Deletes every other ref under the prefix when the repository opens.
    Sweep { workdir: PathBuf, keep: Vec<u64> },
}

/// Maps pull request `id`'s threads' lines (on `paths`) from the service's head (`head`) onto
/// the local source tip (`local`), the files' tip; `base` is the local diff's base and `target`
/// the target's tip. Reads only.
#[derive(Debug)]
pub struct LineMapJob {
    pub workdir: PathBuf,
    pub id: u64,
    pub head: String,
    pub local: Oid,
    pub base: Oid,
    pub target: Oid,
    pub paths: Vec<String>,
    pub generation: u64,
}

/// Where a service call goes, with this session's credential for it, if any; `Debug` hides
/// the credential (Auth's own `Debug`).
#[derive(Debug)]
pub struct PrRequest {
    pub workdir: PathBuf,
    /// Where signing in asks git's credential helpers (`AppState::credentials_dir_for`).
    pub cwd: PathBuf,
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
        /// The head the page showed; GitLab approves only that head.
        head_sha: String,
    },
    /// Merge; once merged, `source` is deleted on `remote` (when `delete`), `remote` fetched and
    /// the local branch deleted (when `delete_local`).
    Merge {
        req: MergeRequest,
        remote: String,
        source: String,
        delete: bool,
        delete_local: bool,
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
    /// Start a code comment on diff lines, alone.
    LineComment(NewComment),
    /// Add a code comment to the user's pending review (`pending`, when the page knows it),
    /// starting one when there is none.
    ReviewComment {
        comment: NewComment,
        pending: Option<u64>,
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
    /// The file list and commits, from local refs (scoped to its repository; reads only).
    Files(FilesJob),
    /// The service's head ref, fetched into `refs/glowfly/pr/<id>` (scoped; writes only that ref).
    FetchHead(HeadJob),
    /// Keeping or deleting those refs (no result).
    HeadRef(HeadRefJob),
    /// A call to the hosting service (unscoped: its reply names its repository).
    Call(Box<CallJob>),
    /// The threads' lines mapped onto the local source tip (scoped; reads only).
    LineMaps(LineMapJob),
}

impl EffectMeta for PrEffect {
    fn name(&self) -> &'static str {
        match self {
            PrEffect::Files(_) => "load-pr-files",
            PrEffect::FetchHead(_) => "fetch-pr-head",
            PrEffect::LineMaps(_) => "map-pr-lines",
            PrEffect::HeadRef(HeadRefJob::Keep { .. }) => "keep-pr-head",
            PrEffect::HeadRef(HeadRefJob::Drop { .. }) => "drop-pr-heads",
            PrEffect::HeadRef(HeadRefJob::Sweep { .. }) => "sweep-pr-heads",
            PrEffect::Call(j) => match j.call {
                PrCall::Load => "load-pr-page",
                PrCall::Checks { .. } => "load-pr-checks",
                PrCall::Comment(_) => "post-pr-comment",
                PrCall::Review { .. } => "submit-pr-review",
                PrCall::Merge { .. } => "merge-pr",
                PrCall::Reply { .. } => "reply-pr-thread",
                PrCall::Resolve { .. } => "resolve-pr-thread",
                PrCall::LineComment(_) => "post-pr-line-comment",
                PrCall::ReviewComment { .. } => "add-pr-review-comment",
            },
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            PrEffect::Files(j) => Some(&j.workdir),
            PrEffect::FetchHead(j) => Some(&j.workdir),
            PrEffect::LineMaps(j) => Some(&j.workdir),
            PrEffect::Call(_) | PrEffect::HeadRef(_) => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Reads, calls, and a pull request head fetch, which writes only Glowfly's own refs under
    /// `refs/glowfly/pr/` that no other work reads or writes; a long job never delays them.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        match self {
            // Local libgit2 work on a test's repository.
            PrEffect::HeadRef(_) => SandboxRule::Run,
            _ => SandboxRule::Drop,
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
        delete_local: bool,
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
    /// A new code comment's thread, alone or in the pending review (`review`).
    LineCommented {
        review: bool,
        result: Result<NewThread, ForgeError>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PullRequestMsg {
    /// Open the panel for pull request .0 (the sidebar row's double-click and menu).
    Open(u64),
    /// Close the panel (its Close button, Esc).
    Close,
    /// The files and commits of pull request `id` (the generation tells a stale answer).
    FilesLoaded { id: u64, generation: u64, result: Result<PrCompare, String> },
    /// Show Overview, Commits or Files changed.
    Tab(PrTab),
    /// A Commits row was clicked: show that commit in the commit panel.
    SelectCommit(Oid),
    /// Fetch the pull request's head ref, with prompts (the notes' button).
    FetchHead,
    /// The threads' lines of pull request `id` mapped onto the local source tip (the generation
    /// tells a stale answer).
    LinesMapped { id: u64, generation: u64, result: Result<PrLineMaps, String> },
    /// The head ref fetch for pull request `id`, which should have brought `head`.
    HeadFetched { id: u64, head: String, result: Result<(), String> },
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
    /// A new code comment on diff lines.
    LineComment(LineCommentMsg),
    /// A service call's answer, for pull request `id` of `target` in `workdir`.
    Replied { workdir: PathBuf, target: Target, id: u64, generation: u64, reply: Box<PrReply> },
}

impl PullRequestMsg {
    /// Nothing is kept for an inactive tab, except that a HeadFetched registers the ref it may
    /// have written (`Inactive::PrHeadFetched`), which the settle pass deletes once no panel
    /// shows it: intents (the thread and Checks ones too) are for the
    /// active tab; a dropped FilesLoaded is asked for again by `reload_pending` when its tab
    /// becomes active (as are the checks); a dropped LinesMapped leaves its state Loading, so
    /// reload_pending asks again; a dropped HeadFetched leaves its fetch running, so
    /// `reload_pending` starts it again (the fetch's ref is registered meanwhile); a new code comment's intents (LineComment) are for the
    /// active tab too, their answers come in Replied, which its handler applies to a parked
    /// panel; and Replied comes from an unscoped effect, so it never arrives scoped (its handler matches
    /// the repository itself and logs results for other tabs).
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::HeadFetched { id, .. } => Some(crate::msg::Inactive::PrHeadFetched(id)),
            Self::Open(_)
            | Self::Close
            | Self::FilesLoaded { .. }
            | Self::Tab(_)
            | Self::SelectCommit(_)
            | Self::FetchHead
            | Self::LinesMapped { .. }
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
            | Self::LineComment(_)
            | Self::Replied { .. } => None,
        }
    }
}
