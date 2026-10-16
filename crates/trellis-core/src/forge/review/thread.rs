//! Code comment threads of a pull request (review threads): comments made on a line or a whole
//! file of the changes, with replies, read with the page and answered or resolved from the
//! pull request panel.

use super::super::{Auth, Target, links};
use super::{github_threads, reviews_in_app, unsupported};
use crate::diff_rows::LineSide;
use crate::error::Result;

/// One comment of a thread.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadComment {
    /// The comment's REST id, which replies are posted to; `None` when the service sent none.
    pub id: Option<u64>,
    pub author: String,
    /// Unix seconds; 0 when the service sent no readable time.
    pub time: i64,
    pub body: String,
    /// Web page; empty when the service sent no usable link.
    pub url: String,
    /// Part of the signed-in user's review that isn't submitted yet: only they can see it.
    pub pending: bool,
}

/// A thread of comments on one line, a range of lines or the whole of a changed file.
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewThread {
    /// The service's id for the thread, which resolving takes.
    pub id: String,
    /// The file, as the service spells it (repository-relative, `/`-separated).
    pub path: String,
    /// Whether the line numbers count in the old file (a removed line) or the new one.
    pub side: LineSide,
    /// The (last) line commented on in the current changes; `None` for a comment on the whole
    /// file, and for an outdated one.
    pub line: Option<u32>,
    /// The first line of a comment on several lines.
    pub start_line: Option<u32>,
    /// The line in the version the comment was made on, for an outdated thread.
    pub original_line: Option<u32>,
    /// The code commented on has changed since.
    pub outdated: bool,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub can_reply: bool,
    pub can_resolve: bool,
    pub can_unresolve: bool,
    /// The last few lines of the diff the first comment was made on, as plain text.
    pub context: String,
    /// Oldest first; never empty.
    pub comments: Vec<ThreadComment>,
    /// How many more comments the service has than were read.
    pub more: usize,
}

impl ReviewThread {
    /// When the thread started: its first comment's time.
    pub fn time(&self) -> i64 {
        self.comments.first().map_or(0, |c| c.time)
    }

    /// The comment replies are posted to: the first one.
    pub fn reply_to(&self) -> Option<u64> {
        self.comments.first().and_then(|c| c.id)
    }
}

pub(super) fn checked_comment(t: &Target, mut c: ThreadComment) -> ThreadComment {
    c.url = links::web_url(t, std::mem::take(&mut c.url));
    c
}

pub(super) fn checked_thread(t: &Target, mut th: ReviewThread) -> ReviewThread {
    th.comments = th.comments.into_iter().map(|c| checked_comment(t, c)).collect();
    th
}

/// Replies to thread `comment` (its first comment's id) of pull request `id`; returns the reply
/// as the service stored it.
pub fn reply_to_thread(t: &Target, auth: &Auth, id: u64, comment: u64, body: &str) -> Result<ThreadComment> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    github_threads::reply(t, auth, id, comment, body).map(|c| checked_comment(t, c))
}

/// Resolves (`resolve`) or reopens the thread with service id `thread`; returns whether it is
/// resolved now.
pub fn resolve_thread(t: &Target, auth: &Auth, thread: &str, resolve: bool) -> Result<bool> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    github_threads::resolve(t, auth, thread, resolve)
}
