//! A pull request's conversation, reviews and merging on its hosting service: the page the
//! pull request panel shows (state, mergeability, the repository's merge settings, comments and
//! submitted reviews, code comment threads), posting a comment, submitting a review, replying
//! to and resolving threads (`thread.rs`), starting a code comment on diff lines, alone or in
//! the user's pending review (`line_anchor.rs`, `new_comment.rs`), and merging. `route.rs` picks
//! the service: GitHub and GitHub Enterprise Server (github.rs, github_threads.rs), GitLab
//! (gitlab.rs), Bitbucket Cloud (bitbucket.rs) and Bitbucket Data Center (bitbucket_dc.rs), with
//! both Bitbucket services' merging in bitbucket_merge.rs; only GitHub has code comment threads
//! and new code comments. Every other service answers
//! `Error::Unsupported` without sending anything.

use super::{Target, links};
use crate::error::Error;

mod bitbucket;
mod bitbucket_dc;
mod bitbucket_merge;
mod github;
mod github_new_comment;
pub(super) mod github_threads;
mod gitlab;
mod line_anchor;
mod new_comment;
mod route;
mod thread;

pub use line_anchor::*;
pub use new_comment::*;
pub use route::*;
pub use thread::*;

/// The start of the error for a merge or approval refused because the source branch moved;
/// the caller ends it with the action (" merge.").
pub(super) const MOVED: &str = "The source branch changed on the service since this was loaded. Reload to see the new commits, then";

/// The error for merging a pull request that the service no longer has open.
pub(super) const NOT_OPEN_MERGE: &str = "This pull request isn't open any more. Reload to see its state.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
    /// The target moves to the source's last commit, with no merge commit (GitLab, Bitbucket).
    FastForward,
}

impl MergeMethod {
    /// GitHub's merge methods, in its listing order.
    pub const GITHUB: [MergeMethod; 3] = [MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase];

    /// The name the service's API and Glowfly's log use.
    pub fn api_name(self) -> &'static str {
        match self {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
            MergeMethod::FastForward => "fast-forward",
        }
    }
}

/// Whether the service would merge the pull request now, and if not, why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeState {
    Clean,
    Conflicts,
    Blocked,
    Behind,
    Unstable,
    Unknown,
}

/// How the service reviews and merges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStyle {
    /// GitHub: comment, approve or request changes, each a submitted review; every merge
    /// method is listed, the ones the repository turns off disabled.
    Verdicts,
    /// GitLab: approvals that can be taken back and comments as notes; only the project's
    /// merge methods are listed.
    Approvals {
        /// The signed-in user has approved.
        by_me: bool,
        /// Whether the service lets the signed-in user approve, when said.
        can_approve: Option<bool>,
    },
    /// Bitbucket: approve or request changes, each taken back once given, and comments as
    /// comments; only the repository's merge strategies are listed.
    Votes { mine: Vote },
}

/// The signed-in user's vote on a Bitbucket pull request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vote {
    Neither,
    Approved,
    ChangesRequested,
}

/// The pull request as its page on the service has it, with the settings that decide how it
/// can be reviewed and merged.
#[derive(Clone, Debug, PartialEq)]
pub struct PrDetail {
    pub state: PrState,
    pub draft: bool,
    /// The conversation is locked to collaborators.
    pub locked: bool,
    pub mergeable: Option<bool>,
    pub merge_state: MergeState,
    /// The source's head commit, in full on every service.
    pub head_sha: String,
    /// The target's commit the changes are compared from; on GitLab, the merge base the
    /// service diffed from.
    pub base_sha: String,
    /// The source branch is in the same repository (false for forks, and when the fork was
    /// deleted).
    pub same_repo: bool,
    pub author: String,
    /// The signed-in user's login.
    pub me: String,
    /// Whether the signed-in user may push to (and so merge into) the repository, when said.
    pub can_merge: Option<bool>,
    /// The merge methods the service allows, in its listing order (GitHub: `MergeMethod::GITHUB`
    /// order; GitLab and Bitbucket: the repository's default first).
    pub methods: Vec<MergeMethod>,
    /// The repository deletes a merged pull request's branch itself.
    pub deletes_merged_branches: bool,
    /// The signed-in user's review that is started but not yet submitted.
    pub pending_review: Option<u64>,
    pub style: ReviewStyle,
}

impl PrDetail {
    /// Whether the signed-in user opened the pull request.
    pub fn is_author(&self) -> bool {
        !self.me.is_empty() && self.me.eq_ignore_ascii_case(&self.author)
    }
}

/// What a submitted review said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
    /// An approval was taken back (GitLab).
    ApprovalRemoved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Comment,
    Review(Verdict),
}

/// One comment or submitted review in the conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub author: String,
    /// Unix seconds; 0 when the service sent no readable time.
    pub time: i64,
    pub kind: EntryKind,
    pub body: String,
    /// Web page; empty when the service sent no usable link.
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrPage {
    pub detail: PrDetail,
    /// Oldest first.
    pub entries: Vec<Entry>,
    /// Code comment threads, ordered by their first comment.
    pub threads: Vec<ReviewThread>,
    /// Why some or all threads are missing (a server that can't list them), when they are.
    pub threads_note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewEvent {
    Comment,
    Approve,
    RequestChanges,
    /// Take back the signed-in user's approval (GitLab, Bitbucket).
    Unapprove,
    /// Take back the signed-in user's request for changes (Bitbucket).
    UnrequestChanges,
}

impl ReviewEvent {
    pub fn api_name(self) -> &'static str {
        match self {
            ReviewEvent::Comment => "COMMENT",
            ReviewEvent::Approve => "APPROVE",
            ReviewEvent::RequestChanges => "REQUEST_CHANGES",
            ReviewEvent::Unapprove => "UNAPPROVE",
            ReviewEvent::UnrequestChanges => "UNREQUEST_CHANGES",
        }
    }
}

/// A review to submit.
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewRequest {
    pub event: ReviewEvent,
    /// The summary; may be empty.
    pub body: String,
    /// The signed-in user's review started on the service, to finish (GitHub).
    pub pending: Option<u64>,
    /// The head the user saw; GitLab refuses an approval when the branch has moved since.
    pub head_sha: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeRequest {
    pub method: MergeMethod,
    /// The head the user saw; the service refuses the merge when the branch has moved since.
    pub head_sha: String,
    /// Empty: the service's default.
    pub title: String,
    /// Empty: the service's default.
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    pub sha: String,
}

pub(super) fn unsupported(t: &Target) -> Error {
    Error::Unsupported(format!("{} reviews aren't supported in Glowfly yet", t.provider.name()))
}

pub(super) fn checked(t: &Target, mut e: Entry) -> Entry {
    e.url = links::web_url(t, std::mem::take(&mut e.url));
    e
}

/// The commit message a service takes: the title, a blank line and the message, either left
/// out when empty.
pub(super) fn message_of(req: &MergeRequest) -> String {
    [req.title.trim(), req.message.trim()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(
        "

",
    )
}

/// Whether two commit ids name the same commit: true when either is empty (nothing to compare),
/// or when the shorter, at least 7 characters long, starts the longer (ignoring case).
pub(super) fn same_commit(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 7 && long.get(..short.len()).is_some_and(|p| p.eq_ignore_ascii_case(short))
}
