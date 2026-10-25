//! Bitbucket Cloud pull requests in the panel: the page (state, draft, head, conflicts from the
//! diffstat, the destination branch's merge strategies, the signed-in user's vote), general
//! comments and votes oldest first, posting a comment, and approving or requesting changes and
//! taking either back; inline comments are counted and left to the browser.

use super::super::bitbucket::{CLOUD, CloudUser, Href, cloud_list, cloud_repo};
use super::super::filters::bitbucket::me;
use super::super::http::delete;
use super::super::issue_details::api_time;
use super::super::links::https_host;
use super::super::{Auth, Target, enc, get, page, post};
use super::bitbucket_merge::cloud_methods;
use super::{Entry, EntryKind, MergeState, PrDetail, PrPage, PrState, ReviewEvent, ReviewRequest, ReviewStyle, Verdict, Vote, same_commit};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiPr {
    state: String,
    draft: bool,
    close_source_branch: Option<bool>,
    author: Option<CloudUser>,
    participants: Vec<ApiParticipant>,
    source: End,
    destination: End,
    links: ApiLinks,
}

/// One side of the pull request: its branch, commit and repository.
#[derive(Deserialize, Default)]
#[serde(default)]
struct End {
    branch: EndBranch,
    commit: Option<Commit>,
    repository: Option<Repo>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EndBranch {
    merge_strategies: Option<Vec<String>>,
    default_merge_strategy: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Commit {
    hash: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Repo {
    full_name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiLinks {
    html: Option<Href>,
    diffstat: Option<Href>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiParticipant {
    user: CloudUser,
    approved: bool,
    state: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Raw {
    raw: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Parent {
    id: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiComment {
    id: u64,
    content: Option<Raw>,
    user: Option<CloudUser>,
    created_on: String,
    deleted: bool,
    inline: Option<Value>,
    parent: Option<Parent>,
    links: ApiLinks,
}

/// A vote in the activity stream.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Cast {
    date: String,
    user: Option<CloudUser>,
}

/// One activity item; only votes are read (comments come from their own list).
#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiActivity {
    approval: Option<Cast>,
    changes_requested: Option<Cast>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DiffStat {
    status: String,
}

pub(super) fn pr_api(t: &Target, id: u64) -> String {
    format!("{}/pullrequests/{id}", cloud_repo(t))
}

/// DECLINED and SUPERSEDED pull requests are closed.
fn pr_state(state: &str) -> PrState {
    match state {
        "OPEN" => PrState::Open,
        "MERGED" => PrState::Merged,
        _ => PrState::Closed,
    }
}

/// The signed-in user's vote, from their participant entry.
fn my_vote(participants: &[ApiParticipant], uuid: &str) -> Vote {
    if uuid.is_empty() {
        return Vote::Neither;
    }
    let Some(p) = participants.iter().find(|p| p.user.uuid.eq_ignore_ascii_case(uuid)) else { return Vote::Neither };
    match (p.approved, p.state.as_deref()) {
        (true, _) | (_, Some("approved")) => Vote::Approved,
        (_, Some("changes_requested")) => Vote::ChangesRequested,
        _ => Vote::Neither,
    }
}

fn name(u: Option<CloudUser>) -> String {
    u.map(|u| u.display_name).unwrap_or_default()
}

fn comment_entry(c: ApiComment) -> Entry {
    Entry {
        author: name(c.user),
        time: api_time(&c.created_on),
        kind: EntryKind::Comment,
        body: c.content.map(|c| c.raw).unwrap_or_default(),
        url: c.links.html.map(|h| h.href).unwrap_or_default(),
    }
}

/// The general comments (replies among them, flat) in the API's order, and how many code
/// comments (with their replies) were left out. Deleted comments are dropped.
fn comment_entries(comments: Vec<ApiComment>) -> (Vec<Entry>, usize) {
    let mut on_code = HashSet::new();
    let (mut out, mut counted) = (Vec::new(), 0);
    for c in comments {
        let inline = c.inline.is_some() || c.parent.as_ref().is_some_and(|p| on_code.contains(&p.id));
        if inline {
            on_code.insert(c.id);
        }
        if c.deleted {
            continue;
        }
        if inline {
            counted += 1;
        } else {
            out.push(comment_entry(c));
        }
    }
    (out, counted)
}

/// Approvals and change requests as review entries linking to the pull request's page.
fn vote_entries(activity: Vec<ApiActivity>, page_url: &str) -> Vec<Entry> {
    let review = |cast: Cast, v: Verdict| Entry {
        author: name(cast.user),
        time: api_time(&cast.date),
        kind: EntryKind::Review(v),
        body: String::new(),
        url: page_url.to_owned(),
    };
    let mut out = Vec::new();
    for a in activity {
        if let Some(c) = a.approval {
            out.push(review(c, Verdict::Approved));
        }
        if let Some(c) = a.changes_requested {
            out.push(review(c, Verdict::ChangesRequested));
        }
    }
    out
}

fn conflicted(files: &[DiffStat]) -> bool {
    files.iter().any(|f| f.status == "merge conflict")
}

fn hidden(e: &Error) -> bool {
    e.is_not_found() || matches!(e, Error::Forbidden(_))
}

/// The full head commit id for the 12-character `prefix` Bitbucket lists; the prefix when the
/// source repository can't be read.
fn full_head(auth: &Auth, full_name: &str, prefix: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Found {
        #[serde(default)]
        hash: String,
    }
    let Some((ws, repo)) = full_name.split_once('/') else { return Ok(prefix.to_owned()) };
    let url = format!("{CLOUD}/repositories/{}/{}/commit/{}?fields=hash", enc(ws), enc(repo), enc(prefix));
    match get::<Found>(auth, &url) {
        Ok(f) if !f.hash.is_empty() && same_commit(prefix, &f.hash) => Ok(f.hash),
        Ok(_) => Ok(prefix.to_owned()),
        Err(e) if hidden(&e) => Ok(prefix.to_owned()),
        Err(e) => Err(e),
    }
}

/// Conflicts from the diffstat's file statuses; Unknown when it can't be read.
fn merge_state(auth: &Auth, diffstat: Option<&Href>) -> Result<MergeState> {
    let Some(href) = diffstat.map(|h| h.href.as_str()).filter(|h| https_host(h).as_deref() == Some("api.bitbucket.org")) else {
        return Ok(MergeState::Unknown);
    };
    match cloud_list::<DiffStat>(auth, page::with_param(href, "pagelen", "100")) {
        Ok(files) if conflicted(&files) => Ok(MergeState::Conflicts),
        Ok(_) => Ok(MergeState::Clean),
        Err(e) if hidden(&e) => Ok(MergeState::Unknown),
        Err(e) => Err(e),
    }
}

fn repo_name(end: &End) -> Option<&str> {
    end.repository.as_ref().map(|r| r.full_name.as_str()).filter(|n| !n.is_empty())
}

pub(super) fn pr_page(t: &Target, auth: &Auth, id: u64) -> Result<PrPage> {
    let uuid = me(auth)?.id;
    let api = pr_api(t, id);
    let pr: ApiPr = get(auth, &api)?;
    let prefix = pr.source.commit.as_ref().map(|c| c.hash.clone()).unwrap_or_default();
    let head_sha = match repo_name(&pr.source) {
        Some(full) if !prefix.is_empty() => full_head(auth, full, &prefix)?,
        _ => prefix,
    };
    let state = pr_state(&pr.state);
    let merge_state = if state == PrState::Open { merge_state(auth, pr.links.diffstat.as_ref())? } else { MergeState::Unknown };
    let comments: Vec<ApiComment> = cloud_list(auth, format!("{api}/comments?pagelen=100&sort=created_on"))?;
    let activity: Vec<ApiActivity> = cloud_list(auth, format!("{api}/activity?pagelen=50"))?;

    let same_repo = match (repo_name(&pr.source), repo_name(&pr.destination)) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    let branch = &pr.destination.branch;
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
        head_sha,
        base_sha: pr.destination.commit.as_ref().map(|c| c.hash.clone()).unwrap_or_default(),
        same_repo,
        author: pr.author.as_ref().map(|a| a.uuid.clone()).unwrap_or_default(),
        style: ReviewStyle::Votes { mine: my_vote(&pr.participants, &uuid) },
        me: uuid,
        can_merge: None,
        methods: cloud_methods(branch.merge_strategies.as_deref(), branch.default_merge_strategy.as_deref()),
        deletes_merged_branches: pr.close_source_branch.unwrap_or(false),
        pending_review: None,
    };
    let page_url = pr.links.html.map(|h| h.href).unwrap_or_default();
    let (mut entries, inline) = comment_entries(comments);
    entries.extend(vote_entries(activity, &page_url));
    entries.sort_by_key(|e| e.time);
    let threads_note = (inline > 0)
        .then(|| format!("Code comments on Bitbucket pull requests open in the browser for now ({inline} on this pull request)."));
    Ok(PrPage { detail, entries, threads: Vec::new(), threads_note })
}

pub(super) fn post_comment(t: &Target, auth: &Auth, id: u64, body: &str) -> Result<Entry> {
    let c: ApiComment = post(auth, &format!("{}/comments", pr_api(t, id)), json!({ "content": { "raw": body } }))?;
    Ok(comment_entry(c))
}

/// A vote call's failure in plain words; Cloud's 401 is bad credentials and stays a sign-in
/// problem.
fn refused(e: Error, ev: ReviewEvent) -> Error {
    match (e, ev) {
        (Error::NotFound(_), ReviewEvent::Unapprove) => {
            Error::Other("You haven't approved this pull request. Reload to see its reviews.".into())
        }
        (Error::NotFound(_), ReviewEvent::UnrequestChanges) => {
            Error::Other("You haven't requested changes on this pull request. Reload to see its reviews.".into())
        }
        (e, _) => e,
    }
}

pub(super) fn submit_review(t: &Target, auth: &Auth, id: u64, req: &ReviewRequest) -> Result<()> {
    let api = pr_api(t, id);
    let comment = |body: &str| post::<Value>(auth, &format!("{api}/comments"), json!({ "content": { "raw": body } })).map(drop);
    let vote = |path: &str| post::<Value>(auth, &format!("{api}/{path}"), json!({})).map(drop);
    let unvote = |path: &str| delete(auth, &format!("{api}/{path}"));
    let (sent, done) = match req.event {
        ReviewEvent::Comment => return comment(&req.body),
        ReviewEvent::Approve => (vote("approve"), "Approved"),
        ReviewEvent::RequestChanges => (vote("request-changes"), "Requested changes"),
        ReviewEvent::Unapprove => (unvote("approve"), "Removed your approval"),
        ReviewEvent::UnrequestChanges => (unvote("request-changes"), "Removed your change request"),
    };
    sent.map_err(|e| refused(e, req.event))?;
    if req.body.trim().is_empty() {
        return Ok(());
    }
    comment(&req.body).map_err(|e| Error::Other(format!("{done}, but the summary wasn't posted: {e}")))
}

#[cfg(test)]
mod tests;
