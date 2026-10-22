//! Pull request conversations, reviews and merging on GitHub and GitHub Enterprise Server
//! (REST API v3); code comment threads are in github_threads.rs.

use super::super::github::{me, repo_api};
use super::super::http::put;
use super::super::{Auth, Target, get, page, post};
use super::{
    Entry, EntryKind, MergeMethod, MergeRequest, MergeState, Merged, PrDetail, PrPage, PrState, ReviewEvent, ReviewRequest, ReviewStyle,
    Verdict,
};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;

/// How a deleted account is shown.
const GHOST: &str = "ghost";

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Push {
    #[serde(default)]
    push: bool,
}

#[derive(Deserialize)]
struct ApiRepo {
    #[serde(default)]
    allow_merge_commit: Option<bool>,
    #[serde(default)]
    allow_squash_merge: Option<bool>,
    #[serde(default)]
    allow_rebase_merge: Option<bool>,
    #[serde(default)]
    delete_branch_on_merge: Option<bool>,
    #[serde(default)]
    permissions: Option<Push>,
}

#[derive(Deserialize)]
struct FullName {
    full_name: String,
}

#[derive(Deserialize)]
struct Head {
    #[serde(default)]
    sha: String,
    #[serde(default)]
    repo: Option<FullName>,
}

#[derive(Deserialize)]
struct ApiPull {
    #[serde(default)]
    state: String,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    mergeable: Option<bool>,
    #[serde(default)]
    mergeable_state: Option<String>,
    user: Option<Login>,
    head: Head,
    base: Head,
}

#[derive(Deserialize)]
struct ApiComment {
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    html_url: String,
}

#[derive(Deserialize)]
struct ApiReview {
    id: u64,
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    submitted_at: Option<String>,
    #[serde(default)]
    html_url: String,
}

#[derive(Deserialize)]
struct ApiMerged {
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    message: Option<String>,
}

fn pull_api(t: &Target, id: u64) -> String {
    format!("{}/pulls/{id}", repo_api(t))
}

pub(super) fn time(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp()).unwrap_or(0)
}

fn author(user: Option<Login>) -> String {
    user.map_or_else(|| GHOST.to_owned(), |u| u.login)
}

pub(super) fn merge_state(mergeable: Option<bool>, state: Option<&str>) -> MergeState {
    let s = match state.unwrap_or_default() {
        "clean" | "has_hooks" => MergeState::Clean,
        "dirty" => MergeState::Conflicts,
        "blocked" => MergeState::Blocked,
        "behind" => MergeState::Behind,
        "unstable" => MergeState::Unstable,
        _ => MergeState::Unknown,
    };
    if mergeable == Some(false) && s != MergeState::Blocked { MergeState::Conflicts } else { s }
}

pub(super) fn pr_state(state: &str, merged: bool) -> PrState {
    match (merged, state) {
        (true, _) => PrState::Merged,
        (false, "closed") => PrState::Closed,
        _ => PrState::Open,
    }
}

fn comment_entry(c: ApiComment) -> Entry {
    Entry { author: author(c.user), time: time(&c.created_at), kind: EntryKind::Comment, body: c.body.unwrap_or_default(), url: c.html_url }
}

/// A submitted review as an entry; None for a pending (or unknown) one.
fn review_entry(r: ApiReview) -> Option<Entry> {
    let verdict = match r.state.as_str() {
        "APPROVED" => Verdict::Approved,
        "CHANGES_REQUESTED" => Verdict::ChangesRequested,
        "COMMENTED" => Verdict::Commented,
        "DISMISSED" => Verdict::Dismissed,
        _ => return None,
    };
    Some(Entry {
        author: author(r.user),
        time: r.submitted_at.as_deref().map_or(0, time),
        kind: EntryKind::Review(verdict),
        body: r.body.unwrap_or_default(),
        url: r.html_url,
    })
}

/// The signed-in user's pending review among `reviews`.
fn pending_of(reviews: &[ApiReview], me: &str) -> Option<u64> {
    reviews.iter().find(|r| r.state == "PENDING" && r.user.as_ref().is_some_and(|u| u.login.eq_ignore_ascii_case(me))).map(|r| r.id)
}

pub(super) fn pr_page(t: &Target, auth: &Auth, id: u64) -> Result<PrPage> {
    let me = me(&t.host, t.provider, auth)?.login;
    let repo: ApiRepo = get(auth, &repo_api(t))?;
    let pull: ApiPull = get(auth, &pull_api(t, id))?;
    let comments: Vec<ApiComment> = page::by_link(auth, format!("{}/issues/{id}/comments?per_page=100", repo_api(t)))?;
    let reviews: Vec<ApiReview> = page::by_link(auth, format!("{}/reviews?per_page=100", pull_api(t, id)))?;

    let allowed = |flag: Option<bool>| flag.unwrap_or(true);
    let flags = [repo.allow_merge_commit, repo.allow_squash_merge, repo.allow_rebase_merge];
    let methods = MergeMethod::GITHUB.into_iter().zip(flags).filter(|(_, f)| allowed(*f)).map(|(m, _)| m).collect();
    let base_repo = pull.base.repo.as_ref().map(|r| r.full_name.to_lowercase());
    let head_repo = pull.head.repo.as_ref().map(|r| r.full_name.to_lowercase());
    let detail = PrDetail {
        state: pr_state(&pull.state, pull.merged),
        draft: pull.draft,
        locked: pull.locked,
        mergeable: pull.mergeable,
        merge_state: merge_state(pull.mergeable, pull.mergeable_state.as_deref()),
        same_repo: head_repo.is_some() && head_repo == base_repo,
        head_sha: pull.head.sha,
        base_sha: pull.base.sha,
        author: author(pull.user),
        can_merge: repo.permissions.map(|p| p.push),
        methods,
        deletes_merged_branches: repo.delete_branch_on_merge.unwrap_or(false),
        pending_review: pending_of(&reviews, &me),
        me,
        style: ReviewStyle::Verdicts,
    };
    let mut entries: Vec<Entry> = comments.into_iter().map(comment_entry).chain(reviews.into_iter().filter_map(review_entry)).collect();
    entries.sort_by_key(|e| e.time);
    let (threads, threads_note) = super::github_threads::threads(t, auth, id)?;
    Ok(PrPage { detail, entries, threads, threads_note })
}

pub(super) fn post_comment(t: &Target, auth: &Auth, id: u64, body: &str) -> Result<Entry> {
    let c: ApiComment = post(auth, &format!("{}/issues/{id}/comments", repo_api(t)), json!({ "body": body }))?;
    Ok(comment_entry(c))
}

pub(super) fn submit_review(t: &Target, auth: &Auth, id: u64, req: &ReviewRequest) -> Result<()> {
    if req.event == ReviewEvent::Unapprove {
        return Err(Error::Unsupported("GitHub has no approvals to take back".into()));
    }
    let url = match req.pending {
        Some(p) => format!("{}/reviews/{p}/events", pull_api(t, id)),
        None => format!("{}/reviews", pull_api(t, id)),
    };
    post::<serde_json::Value>(auth, &url, json!({ "event": req.event.api_name(), "body": req.body })).map(drop)
}

pub(super) fn merge(t: &Target, auth: &Auth, id: u64, req: &MergeRequest) -> Result<Merged> {
    if req.method == MergeMethod::FastForward {
        return Err(Error::Unsupported("GitHub merges with a merge commit, a squash or a rebase".into()));
    }
    let mut body = json!({ "merge_method": req.method.api_name(), "sha": req.head_sha });
    if !matches!(req.method, MergeMethod::Rebase | MergeMethod::FastForward) {
        if !req.title.is_empty() {
            body["commit_title"] = json!(req.title);
        }
        if !req.message.is_empty() {
            body["commit_message"] = json!(req.message);
        }
    }
    let answer: ApiMerged = put(auth, &format!("{}/merge", pull_api(t, id)), body).map_err(|e| match e {
        Error::Http { status: 409, .. } => Error::Other(
            "The source branch changed on the service since this was loaded. Reload to see the new commits, then merge.".into(),
        ),
        e => e,
    })?;
    if !answer.merged {
        let why = answer.message.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "The service didn't merge it.".to_owned());
        return Err(Error::Other(crate::text::printable(&why)));
    }
    Ok(Merged { sha: answer.sha.unwrap_or_default() })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// An open pull request's detail, for tests of the rules built on it.
    pub(in super::super) fn detail() -> PrDetail {
        PrDetail {
            state: PrState::Open,
            draft: false,
            locked: false,
            mergeable: Some(true),
            merge_state: MergeState::Clean,
            head_sha: "h".into(),
            base_sha: "b".into(),
            same_repo: true,
            author: "ada".into(),
            me: "grace".into(),
            can_merge: Some(true),
            methods: MergeMethod::GITHUB.to_vec(),
            deletes_merged_branches: false,
            pending_review: None,
            style: ReviewStyle::Verdicts,
        }
    }

    #[test]
    fn mergeable_state_mapping() {
        use MergeState::*;
        let cases = [
            (Some(true), Some("clean"), Clean),
            (Some(true), Some("has_hooks"), Clean),
            (Some(false), Some("dirty"), Conflicts),
            (None, Some("blocked"), Blocked),
            (Some(false), Some("blocked"), Blocked),
            (Some(true), Some("behind"), Behind),
            (Some(true), Some("unstable"), Unstable),
            (None, Some("unknown"), Unknown),
            (None, Some("draft"), Unknown),
            (None, None, Unknown),
            (Some(false), Some("unknown"), Conflicts),
            (Some(false), None, Conflicts),
        ];
        for (mergeable, state, want) in cases {
            assert_eq!(merge_state(mergeable, state), want, "{mergeable:?} {state:?}");
        }
    }

    #[test]
    fn state_mapping() {
        assert_eq!(pr_state("closed", true), PrState::Merged);
        assert_eq!(pr_state("closed", false), PrState::Closed);
        assert_eq!(pr_state("open", false), PrState::Open);
        assert_eq!(pr_state("", false), PrState::Open);
    }

    #[test]
    fn review_conversion() {
        let review = |state: &str, user: Option<&str>| ApiReview {
            id: 3,
            user: user.map(|l| Login { login: l.into() }),
            body: None,
            state: state.into(),
            submitted_at: Some("2026-01-02T03:04:05Z".into()),
            html_url: String::new(),
        };
        let kind = |state: &str| review_entry(review(state, Some("ada"))).map(|e| e.kind);
        assert_eq!(kind("APPROVED"), Some(EntryKind::Review(Verdict::Approved)));
        assert_eq!(kind("CHANGES_REQUESTED"), Some(EntryKind::Review(Verdict::ChangesRequested)));
        assert_eq!(kind("COMMENTED"), Some(EntryKind::Review(Verdict::Commented)));
        assert_eq!(kind("DISMISSED"), Some(EntryKind::Review(Verdict::Dismissed)));
        assert_eq!(kind("PENDING"), None);
        let e = review_entry(review("APPROVED", None)).unwrap();
        assert_eq!((e.author.as_str(), e.time, e.body.as_str()), (GHOST, 1_767_323_045, ""));
        let reviews = [review("PENDING", Some("eve")), review("PENDING", Some("Ada"))];
        assert_eq!(pending_of(&reviews, "ada"), Some(3));
        assert_eq!(pending_of(&reviews[..1], "ada"), None);
        assert_eq!(time("not a time"), 0);
    }
}
