//! Merge request pages, notes, approvals and merging on GitLab.com and GitLab Self-Managed
//! (REST API v4); diff discussions aren't read yet (an empty thread list with a note).

use super::super::filters::gitlab::me;
use super::super::gitlab::{User, project_api};
use super::super::gitlab_issues::{ApiNote, note_entry};
use super::super::http::put;
use super::super::{Auth, Target, enc, enc_path, get, links, page, post};
use super::{
    Entry, EntryKind, MergeMethod, MergeRequest, MergeState, Merged, PrDetail, PrPage, PrState, ReviewEvent, ReviewRequest, ReviewStyle,
    Verdict,
};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

/// GitLab's Developer access level, the lowest that may merge into an unprotected branch.
const DEVELOPER: u32 = 30;

const MOVED: &str = "The source branch changed on the service since this was loaded. Reload to see the new commits, then";

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiProject {
    merge_method: Option<String>,
    squash_option: Option<String>,
    remove_source_branch_after_merge: Option<bool>,
    permissions: Option<Permissions>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Permissions {
    project_access: Option<Access>,
    group_access: Option<Access>,
}

#[derive(Deserialize)]
struct Access {
    #[serde(default)]
    access_level: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiMr {
    state: String,
    draft: bool,
    work_in_progress: bool,
    discussion_locked: Option<bool>,
    detailed_merge_status: Option<String>,
    merge_status: Option<String>,
    has_conflicts: bool,
    sha: Option<String>,
    diff_refs: Option<DiffRefs>,
    source_project_id: Option<u64>,
    target_project_id: Option<u64>,
    author: Option<User>,
    user: Option<MrUser>,
    force_remove_source_branch: Option<bool>,
    should_remove_source_branch: Option<bool>,
    web_url: String,
    merge_commit_sha: Option<String>,
    squash_commit_sha: Option<String>,
    merge_error: Option<String>,
}

#[derive(Deserialize)]
struct DiffRefs {
    #[serde(default)]
    base_sha: Option<String>,
}

#[derive(Deserialize)]
struct MrUser {
    #[serde(default)]
    can_merge: Option<bool>,
}

#[derive(Deserialize)]
struct ApprovedBy {
    user: User,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiApprovals {
    user_has_approved: Option<bool>,
    user_can_approve: Option<bool>,
    approved_by: Vec<ApprovedBy>,
}

fn mr_api(t: &Target, iid: u64) -> String {
    format!("{}/merge_requests/{iid}", project_api(t))
}

/// The merge request's web page, which a posted note's link anchors on.
fn mr_url(t: &Target, iid: u64) -> String {
    format!("https://{}/{}/{}/-/merge_requests/{iid}", t.host, enc_path(&t.owner), enc(&t.repo))
}

/// "locked" is GitLab's state while it merges, shown as open.
fn pr_state(state: &str) -> PrState {
    match state {
        "merged" => PrState::Merged,
        "closed" => PrState::Closed,
        _ => PrState::Open,
    }
}

fn merge_state(detailed: Option<&str>, legacy: Option<&str>, has_conflicts: bool) -> MergeState {
    if has_conflicts {
        return MergeState::Conflicts;
    }
    let Some(detailed) = detailed else {
        return match legacy {
            Some("can_be_merged") => MergeState::Clean,
            Some("cannot_be_merged") => MergeState::Conflicts,
            _ => MergeState::Unknown,
        };
    };
    match detailed {
        "mergeable" => MergeState::Clean,
        "conflict" => MergeState::Conflicts,
        "need_rebase" => MergeState::Behind,
        "ci_must_pass"
        | "ci_still_running"
        | "status_checks_must_pass"
        | "security_policy_pipeline_check"
        | "security_policy_violations"
        | "not_approved"
        | "requested_changes"
        | "discussions_not_resolved"
        | "merge_request_blocked"
        | "merge_time"
        | "jira_association_missing"
        | "locked_paths"
        | "locked_lfs_files"
        | "title_regex"
        | "commits_status"
        | "draft_status" => MergeState::Blocked,
        _ => MergeState::Unknown,
    }
}

/// The project's merge methods, its default first.
fn methods(merge_method: Option<&str>, squash: Option<&str>) -> Vec<MergeMethod> {
    let base = if merge_method == Some("ff") { MergeMethod::FastForward } else { MergeMethod::Merge };
    match squash {
        Some("never") => vec![base],
        Some("always") => vec![MergeMethod::Squash],
        Some("default_on") => vec![MergeMethod::Squash, base],
        _ => vec![base, MergeMethod::Squash],
    }
}

/// Whether the signed-in user may merge: the merge request says, else their access level.
fn can_merge(user: Option<&MrUser>, perms: Option<&Permissions>) -> Option<bool> {
    if let Some(can) = user.and_then(|u| u.can_merge) {
        return Some(can);
    }
    let perms = perms?;
    let levels = [&perms.project_access, &perms.group_access];
    levels.into_iter().flatten().map(|a| a.access_level).max().map(|l| l >= DEVELOPER)
}

/// The verdict a system note records, for the two that are approvals.
fn approval_kind(body: &str) -> Option<Verdict> {
    match body.trim() {
        "approved this merge request" => Some(Verdict::Approved),
        "unapproved this merge request" => Some(Verdict::ApprovalRemoved),
        _ => None,
    }
}

/// The conversation's entries in the API's order, and how many code comments were left out.
fn entries(notes: Vec<ApiNote>, page_url: &str) -> (Vec<Entry>, usize) {
    let mut diff_notes = 0;
    let mut out = Vec::new();
    for n in notes {
        if n.system {
            if let Some(v) = approval_kind(&n.body) {
                let mut e = note_entry(n, page_url);
                e.kind = EntryKind::Review(v);
                e.body.clear();
                out.push(e);
            }
        } else if n.note_type.as_deref() == Some("DiffNote") {
            diff_notes += 1;
        } else {
            out.push(note_entry(n, page_url));
        }
    }
    (out, diff_notes)
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

pub(super) fn pr_page(t: &Target, auth: &Auth, iid: u64) -> Result<PrPage> {
    let username = me(t, auth)?.id;
    let project: ApiProject = get(auth, &project_api(t))?;
    let mr: ApiMr = get(auth, &mr_api(t, iid))?;
    let approvals: Option<ApiApprovals> = match get(auth, &format!("{}/approvals", mr_api(t, iid))) {
        Ok(a) => Some(a),
        Err(e) if e.is_not_found() => None,
        Err(e) => return Err(e),
    };
    let notes: Vec<ApiNote> = page::by_link(auth, format!("{}/notes?sort=asc&order_by=created_at&per_page=100", mr_api(t, iid)))?;

    let style = match approvals {
        Some(a) => {
            let listed = a.approved_by.iter().any(|b| !username.is_empty() && b.user.username.eq_ignore_ascii_case(&username));
            ReviewStyle::Approvals { by_me: a.user_has_approved.unwrap_or(listed), can_approve: a.user_can_approve }
        }
        None => ReviewStyle::Approvals { by_me: false, can_approve: None },
    };
    let merge_state = merge_state(mr.detailed_merge_status.as_deref(), mr.merge_status.as_deref(), mr.has_conflicts);
    let deletes = match (mr.force_remove_source_branch, mr.should_remove_source_branch) {
        (None, None) => project.remove_source_branch_after_merge.unwrap_or(false),
        (force, should) => force.unwrap_or(false) || should.unwrap_or(false),
    };
    let detail = PrDetail {
        state: pr_state(&mr.state),
        draft: mr.draft || mr.work_in_progress,
        locked: mr.discussion_locked.unwrap_or(false),
        mergeable: match merge_state {
            MergeState::Clean => Some(true),
            MergeState::Conflicts => Some(false),
            _ => None,
        },
        merge_state,
        head_sha: mr.sha.unwrap_or_default(),
        base_sha: mr.diff_refs.and_then(|d| d.base_sha).unwrap_or_default(),
        same_repo: mr.source_project_id.is_some() && mr.source_project_id == mr.target_project_id,
        author: mr.author.map(|a| a.username).unwrap_or_default(),
        me: username,
        can_merge: can_merge(mr.user.as_ref(), project.permissions.as_ref()),
        methods: methods(project.merge_method.as_deref(), project.squash_option.as_deref()),
        deletes_merged_branches: deletes,
        pending_review: None,
        style,
    };
    let (mut entries, diff_notes) = entries(notes, &links::web_url(t, mr.web_url));
    entries.sort_by_key(|e| e.time);
    let threads_note = (diff_notes > 0)
        .then(|| format!("Code comments on GitLab merge requests open in the browser for now ({diff_notes} on this merge request)."));
    Ok(PrPage { detail, entries, threads: Vec::new(), threads_note })
}

pub(super) fn post_comment(t: &Target, auth: &Auth, iid: u64, body: &str) -> Result<Entry> {
    let n: ApiNote = post(auth, &format!("{}/notes", mr_api(t, iid)), json!({ "body": body }))?;
    Ok(note_entry(n, &mr_url(t, iid)))
}

/// An approval call's failure in plain words; a 401 here is a refusal, not a sign-out.
fn refused(e: Error, ev: ReviewEvent) -> Error {
    match (e, ev) {
        (Error::Http { status: 409, .. }, _) => Error::Other(format!("{MOVED} approve.")),
        (Error::NotFound(_), ReviewEvent::Unapprove) => {
            Error::Other("You haven't approved this merge request. Reload to see its approvals.".into())
        }
        (Error::Auth(_), ReviewEvent::Unapprove) => {
            Error::Forbidden("GitLab didn't take back the approval. Reload to see its approvals.".into())
        }
        (Error::Auth(_), _) => Error::Forbidden(
            "GitLab didn't take the approval: you may have approved it already, or the project's approval rules don't let you. \
             Reload to see its approvals."
                .into(),
        ),
        (e, _) => e,
    }
}

pub(super) fn submit_review(t: &Target, auth: &Auth, iid: u64, req: &ReviewRequest) -> Result<()> {
    let note = |body: &str| post::<ApiNote>(auth, &format!("{}/notes", mr_api(t, iid)), json!({ "body": body })).map(drop);
    let done = match req.event {
        ReviewEvent::Comment => return note(&req.body),
        ReviewEvent::RequestChanges => {
            return Err(Error::Unsupported("GitLab has no Request changes; leave a comment instead".into()));
        }
        ReviewEvent::Approve => {
            let body = if req.head_sha.is_empty() { json!({}) } else { json!({ "sha": req.head_sha }) };
            post::<Value>(auth, &format!("{}/approve", mr_api(t, iid)), body).map_err(|e| refused(e, req.event))?;
            "Approved"
        }
        ReviewEvent::Unapprove => {
            post::<Value>(auth, &format!("{}/unapprove", mr_api(t, iid)), json!({})).map_err(|e| refused(e, req.event))?;
            "Removed your approval"
        }
    };
    if req.body.trim().is_empty() {
        return Ok(());
    }
    note(&req.body).map_err(|e| Error::Other(format!("{done}, but the summary wasn't posted: {e}")))
}

/// The commit message GitLab takes: the title, a blank line and the message, either left out
/// when empty.
fn message_of(req: &MergeRequest) -> String {
    [req.title.trim(), req.message.trim()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n")
}

pub(super) fn merge(t: &Target, auth: &Auth, iid: u64, req: &MergeRequest) -> Result<Merged> {
    let field = match req.method {
        MergeMethod::Rebase => return Err(Error::Unsupported("GitLab merges with a merge commit, a squash or a fast-forward".into())),
        MergeMethod::Merge => Some("merge_commit_message"),
        MergeMethod::Squash => Some("squash_commit_message"),
        MergeMethod::FastForward => None,
    };
    let mut body = json!({ "sha": req.head_sha, "squash": req.method == MergeMethod::Squash });
    let message = message_of(req);
    if let Some(field) = field
        && !message.is_empty()
    {
        body[field] = json!(message);
    }
    let answer: ApiMr = put(auth, &format!("{}/merge", mr_api(t, iid)), body).map_err(|e| match e {
        Error::Http { status: 409, .. } => Error::Other(format!("{MOVED} merge.")),
        Error::Http { status: 405, .. } => Error::Other(
            "GitLab can't merge it yet (it may need approvals, a passing pipeline, resolved threads or a rebase). Reload to see what blocks it."
                .into(),
        ),
        Error::Auth(_) => {
            Error::Forbidden("GitLab says you can't merge this merge request; the target branch's protection decides who may merge.".into())
        }
        e => e,
    })?;
    if answer.state != "merged" {
        let why = answer.merge_error.filter(|m| !m.trim().is_empty());
        let why = why.unwrap_or_else(|| "GitLab didn't merge it. Reload to see its state.".to_owned());
        return Err(Error::Other(crate::text::printable(&why)));
    }
    let sha = non_empty(answer.merge_commit_sha).or(non_empty(answer.squash_commit_sha)).or(non_empty(answer.sha));
    Ok(Merged { sha: sha.unwrap_or_default() })
}

#[cfg(test)]
mod tests;
