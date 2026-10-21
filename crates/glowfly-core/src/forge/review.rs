//! A pull request's conversation, reviews and merging on its hosting service: the page the
//! pull request panel shows (state, mergeability, the repository's merge settings, comments and
//! submitted reviews, code comment threads), posting a comment, submitting a review, replying
//! to and resolving threads (`thread.rs`), and merging. Only GitHub and
//! GitHub Enterprise Server for now; every other service answers `Error::Unsupported` without
//! sending anything.

use super::{Auth, Provider, Target, links};
use crate::error::{Error, Result};

mod github;
pub(super) mod github_threads;
mod thread;

pub use thread::*;

/// Whether comments, reviews and merging work inside Trellis for this service.
pub fn reviews_in_app(p: Provider) -> bool {
    matches!(p, Provider::GitHub | Provider::GitHubEnterprise)
}

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
}

impl MergeMethod {
    pub const ALL: [MergeMethod; 3] = [MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase];

    /// The name the service's API and Trellis's log use.
    pub fn api_name(self) -> &'static str {
        match self {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
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
    pub head_sha: String,
    pub base_sha: String,
    /// The source branch is in the same repository (false for forks, and when the fork was
    /// deleted).
    pub same_repo: bool,
    pub author: String,
    /// The signed-in user's login.
    pub me: String,
    /// Whether the signed-in user may push to (and so merge into) the repository, when said.
    pub can_merge: Option<bool>,
    /// The merge methods the repository allows, in `MergeMethod::ALL` order.
    pub methods: Vec<MergeMethod>,
    /// The repository deletes a merged pull request's branch itself.
    pub deletes_merged_branches: bool,
    /// The signed-in user's review that is started but not yet submitted.
    pub pending_review: Option<u64>,
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
}

impl ReviewEvent {
    pub fn api_name(self) -> &'static str {
        match self {
            ReviewEvent::Comment => "COMMENT",
            ReviewEvent::Approve => "APPROVE",
            ReviewEvent::RequestChanges => "REQUEST_CHANGES",
        }
    }
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

fn unsupported(t: &Target) -> Error {
    Error::Unsupported(format!("{} reviews aren't supported in Trellis yet", t.provider.name()))
}

fn checked(t: &Target, mut e: Entry) -> Entry {
    e.url = links::web_url(t, std::mem::take(&mut e.url));
    e
}

/// The pull request's page: its detail and conversation.
pub fn pr_page(t: &Target, auth: &Auth, id: u64) -> Result<PrPage> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    let mut page = github::pr_page(t, auth, id)?;
    page.entries = page.entries.into_iter().map(|e| checked(t, e)).collect();
    page.threads = page.threads.into_iter().map(|th| thread::checked_thread(t, th)).collect();
    Ok(page)
}

/// Adds a comment to the conversation; returns it as the service stored it.
pub fn post_comment(t: &Target, auth: &Auth, id: u64, body: &str) -> Result<Entry> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    github::post_comment(t, auth, id, body).map(|e| checked(t, e))
}

/// Submits a review, or finishes the user's `pending` one (with its comments).
pub fn submit_review(t: &Target, auth: &Auth, id: u64, event: ReviewEvent, body: &str, pending: Option<u64>) -> Result<()> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    github::submit_review(t, auth, id, event, body, pending)
}

pub fn merge(t: &Target, auth: &Auth, id: u64, req: &MergeRequest) -> Result<Merged> {
    if !reviews_in_app(t.provider) {
        return Err(unsupported(t));
    }
    github::merge(t, auth, id, req)
}

#[cfg(test)]
mod tests {
    use super::super::mock::{Mock, Reply};
    use super::*;

    #[test]
    fn reviews_in_app_only_for_github() {
        for p in Provider::ALL {
            assert_eq!(reviews_in_app(p), matches!(p, Provider::GitHub | Provider::GitHubEnterprise), "{p:?}");
        }
    }

    #[test]
    fn other_services_send_nothing() {
        let m = Mock::start(|_| Reply::not_found());
        for p in [Provider::GitLab, Provider::Bitbucket, Provider::AzureDevOps, Provider::JiraCloud] {
            let t = Target { provider: p, host: "example.com".into(), owner: "o".into(), project: "p".into(), repo: "r".into() };
            let auth = Auth::from_token(p, None, "tok", "test");
            let req = MergeRequest { method: MergeMethod::Merge, head_sha: "abc".into(), title: String::new(), message: String::new() };
            let errors = [
                pr_page(&t, &auth, 1).unwrap_err(),
                post_comment(&t, &auth, 1, "hi").unwrap_err(),
                submit_review(&t, &auth, 1, ReviewEvent::Approve, "", None).unwrap_err(),
                merge(&t, &auth, 1, &req).unwrap_err(),
                reply_to_thread(&t, &auth, 1, 2, "hi").unwrap_err(),
                resolve_thread(&t, &auth, "T_1", true).unwrap_err(),
            ];
            for e in errors {
                assert!(matches!(&e, Error::Unsupported(m) if m.contains(p.name())), "{p:?}: {e}");
            }
        }
        assert!(m.requests().is_empty());
    }

    #[test]
    fn the_author_is_matched_without_case() {
        let mut d = github::tests::detail();
        d.author = "Ada".into();
        d.me = "ada".into();
        assert!(d.is_author());
        d.me = String::new();
        d.author = String::new();
        assert!(!d.is_author(), "an unknown user is nobody's author");
    }
}
