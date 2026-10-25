//! Bitbucket Data Center pull requests in the panel: the page (merge check, the repository's
//! merge strategies, the signed-in user's status), general comments with their replies and
//! votes from the activity stream, posting a comment, and setting the participant status;
//! inline comments are counted and left to the browser.

use super::super::bitbucket::Values;
use super::super::bitbucket_dc::{dc_list, dc_repo, dc_web};
use super::super::filters::bitbucket::dc_me;
use super::super::http::put;
use super::super::{Auth, Target, enc, get, post};
use super::bitbucket_merge::{MergeConfig, dc_methods};
use super::{Entry, EntryKind, MergeState, PrDetail, PrPage, PrState, ReviewEvent, ReviewRequest, ReviewStyle, Verdict, Vote};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcUser {
    name: String,
    slug: String,
    display_name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DcParticipant {
    user: DcUser,
    status: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RepoId {
    id: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcRef {
    latest_commit: String,
    repository: Option<RepoId>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcPr {
    state: String,
    /// Bitbucket 8.18+; older servers have no drafts.
    draft: bool,
    author: DcParticipant,
    reviewers: Vec<DcParticipant>,
    participants: Vec<DcParticipant>,
    from_ref: DcRef,
    to_ref: DcRef,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MergeCheck {
    can_merge: bool,
    conflicted: bool,
    outcome: Option<String>,
    vetoes: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    merge_config: Option<MergeConfig>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcComment {
    id: u64,
    text: String,
    author: DcUser,
    created_date: i64,
    /// The replies, each with its own.
    comments: Vec<DcComment>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Activity {
    action: String,
    created_date: i64,
    user: DcUser,
    comment_action: Option<String>,
    comment: Option<DcComment>,
    /// Present on code comments.
    comment_anchor: Option<Value>,
}

fn pr_api(t: &Target, id: u64) -> String {
    format!("{}/pull-requests/{id}", dc_repo(t))
}

/// The pull request's page on the web, which comment links open.
fn pr_web(t: &Target, id: u64) -> String {
    format!("{}/pull-requests/{id}", dc_web(t))
}

fn pr_state(state: &str) -> PrState {
    match state {
        "OPEN" => PrState::Open,
        "MERGED" => PrState::Merged,
        _ => PrState::Closed,
    }
}

/// The people on the pull request: its author, reviewers and other participants.
fn people(pr: &DcPr) -> impl Iterator<Item = &DcParticipant> {
    std::iter::once(&pr.author).chain(&pr.reviewers).chain(&pr.participants)
}

fn is(u: &DcUser, me: &str) -> bool {
    !me.is_empty() && u.name.eq_ignore_ascii_case(me)
}

/// The signed-in user's vote, from their reviewer or participant status.
fn my_vote(pr: &DcPr, me: &str) -> Vote {
    let status = pr.reviewers.iter().chain(&pr.participants).find(|p| is(&p.user, me)).map(|p| p.status.as_str());
    match status {
        Some("APPROVED") => Vote::Approved,
        Some("NEEDS_WORK") => Vote::ChangesRequested,
        _ => Vote::Neither,
    }
}

/// The signed-in user's slug, which the participants resource is addressed by, when they are
/// on the pull request.
fn my_slug(pr: &DcPr, me: &str) -> Option<String> {
    people(pr).find(|p| is(&p.user, me)).map(|p| p.user.slug.clone()).filter(|s| !s.is_empty())
}

fn merge_state(c: &MergeCheck) -> MergeState {
    if c.conflicted || c.outcome.as_deref() == Some("CONFLICTED") {
        MergeState::Conflicts
    } else if c.can_merge {
        MergeState::Clean
    } else if !c.vetoes.is_empty() {
        MergeState::Blocked
    } else {
        MergeState::Unknown
    }
}

fn comment_entry(c: &DcComment, page: &str) -> Entry {
    Entry {
        author: c.author.display_name.clone(),
        time: c.created_date / 1000,
        kind: EntryKind::Comment,
        body: c.text.clone(),
        url: format!("{page}/overview?commentId={}", c.id),
    }
}

/// `c` and its replies, depth first, without the deleted ones.
fn flatten(c: &DcComment, page: &str, deleted: &HashSet<u64>, out: &mut Vec<Entry>) {
    if !deleted.contains(&c.id) {
        out.push(comment_entry(c, page));
    }
    for r in &c.comments {
        flatten(r, page, deleted, out);
    }
}

/// How many comments in `c`'s thread are left, `c` among them.
fn count(c: &DcComment, deleted: &HashSet<u64>) -> usize {
    usize::from(!deleted.contains(&c.id)) + c.comments.iter().map(|r| count(r, deleted)).sum::<usize>()
}

/// The conversation from the activity stream (newest first): general comments with their
/// replies flat, and votes; and how many code comments were left out.
fn entries(mut activity: Vec<Activity>, page: &str) -> (Vec<Entry>, usize) {
    let commented = |a: &Activity, what: &str| a.action == "COMMENTED" && a.comment_action.as_deref() == Some(what);
    let deleted: HashSet<u64> =
        activity.iter().filter(|a| commented(a, "DELETED")).filter_map(|a| a.comment.as_ref().map(|c| c.id)).collect();
    activity.reverse();
    let (mut out, mut code) = (Vec::new(), 0);
    for a in &activity {
        let verdict = match a.action.as_str() {
            "APPROVED" => Verdict::Approved,
            "UNAPPROVED" => Verdict::ApprovalRemoved,
            "REVIEWED" => Verdict::ChangesRequested,
            _ => {
                if let Some(c) = a.comment.as_ref().filter(|_| commented(a, "ADDED")) {
                    if a.comment_anchor.is_some() {
                        code += count(c, &deleted);
                    } else {
                        flatten(c, page, &deleted, &mut out);
                    }
                }
                continue;
            }
        };
        let url = format!("{page}/overview");
        out.push(Entry {
            author: a.user.display_name.clone(),
            time: a.created_date / 1000,
            kind: EntryKind::Review(verdict),
            body: String::new(),
            url,
        });
    }
    (out, code)
}

fn hidden(e: &Error) -> bool {
    e.is_not_found() || matches!(e, Error::Forbidden(_))
}

pub(super) fn pr_page(t: &Target, auth: &Auth, id: u64) -> Result<PrPage> {
    let me = dc_me(t, auth)?.id;
    let api = pr_api(t, id);
    let pr: DcPr = get(auth, &api)?;
    let state = pr_state(&pr.state);
    let merge_state = match state {
        PrState::Open => match get::<MergeCheck>(auth, &format!("{api}/merge")) {
            Ok(c) => merge_state(&c),
            Err(e) if hidden(&e) => MergeState::Unknown,
            Err(e) => return Err(e),
        },
        _ => MergeState::Unknown,
    };
    // A read: a 401 here is a missing permission, never a reason to sign out.
    let config = match get::<Settings>(auth, &format!("{}/settings/pull-requests", dc_repo(t))) {
        Ok(s) => s.merge_config,
        Err(Error::Auth(_) | Error::Forbidden(_)) => None,
        Err(e) if e.is_not_found() => None,
        Err(e) => return Err(e),
    };
    let activity: Vec<Activity> = dc_list(auth, format!("{api}/activities?limit=100"))?;

    let repo = |r: &DcRef| r.repository.as_ref().and_then(|r| r.id);
    let detail = PrDetail {
        state,
        draft: pr.draft,
        locked: false,
        mergeable: match merge_state {
            MergeState::Clean => Some(true),
            MergeState::Conflicts => Some(false),
            _ => None,
        },
        merge_state,
        head_sha: pr.from_ref.latest_commit.clone(),
        base_sha: pr.to_ref.latest_commit.clone(),
        same_repo: repo(&pr.from_ref).is_some() && repo(&pr.from_ref) == repo(&pr.to_ref),
        author: pr.author.user.name.clone(),
        style: ReviewStyle::Votes { mine: my_vote(&pr, &me) },
        me,
        can_merge: None,
        methods: dc_methods(config.as_ref()),
        deletes_merged_branches: false,
        pending_review: None,
    };
    let (mut entries, code) = entries(activity, &pr_web(t, id));
    entries.sort_by_key(|e| e.time);
    let threads_note =
        (code > 0).then(|| format!("Code comments on Bitbucket pull requests open in the browser for now ({code} on this pull request)."));
    Ok(PrPage { detail, entries, threads: Vec::new(), threads_note })
}

pub(super) fn post_comment(t: &Target, auth: &Auth, id: u64, body: &str) -> Result<Entry> {
    let c: DcComment = post(auth, &format!("{}/comments", pr_api(t, id)), json!({ "text": body }))?;
    Ok(comment_entry(&c, &pr_web(t, id)))
}

/// The signed-in user's slug from the user directory, else their name.
fn find_slug(t: &Target, auth: &Auth, me: &str) -> Result<String> {
    let url = format!("https://{}/rest/api/1.0/users?filter={}&limit=25", t.host, enc(me));
    match get::<Values<DcUser>>(auth, &url) {
        Ok(found) => Ok(found.values.into_iter().find(|u| u.name == me && !u.slug.is_empty()).map_or_else(|| me.to_owned(), |u| u.slug)),
        Err(e) if hidden(&e) => Ok(me.to_owned()),
        Err(e) => Err(e),
    }
}

pub(super) fn submit_review(t: &Target, auth: &Auth, id: u64, req: &ReviewRequest) -> Result<()> {
    let api = pr_api(t, id);
    let comment = |body: &str| post::<Value>(auth, &format!("{api}/comments"), json!({ "text": body })).map(drop);
    let (status, done) = match req.event {
        ReviewEvent::Comment => return comment(&req.body),
        ReviewEvent::Approve => ("APPROVED", "Approved"),
        ReviewEvent::RequestChanges => ("NEEDS_WORK", "Requested changes"),
        ReviewEvent::Unapprove => ("UNAPPROVED", "Removed your approval"),
        ReviewEvent::UnrequestChanges => ("UNAPPROVED", "Removed your change request"),
    };
    let me = dc_me(t, auth)?.id;
    let pr: DcPr = get(auth, &api)?;
    let slug = match my_slug(&pr, &me) {
        Some(s) => s,
        None => find_slug(t, auth, &me)?,
    };
    let body = json!({ "user": { "name": me }, "approved": status == "APPROVED", "status": status });
    put::<Value>(auth, &format!("{api}/participants/{}", enc(&slug)), body).map_err(|e| match e {
        Error::Auth(_) => Error::Forbidden("Bitbucket didn't take the review. Reload to see its reviewers.".into()),
        e => e,
    })?;
    if req.body.trim().is_empty() {
        return Ok(());
    }
    comment(&req.body).map_err(|e| Error::Other(format!("{done}, but the summary wasn't posted: {e}")))
}

#[cfg(test)]
mod tests;
