//! New code comments on GitHub and GitHub Enterprise Server: a single comment through REST, and
//! a comment in the signed-in user's pending review through GraphQL (REST can't add a thread to
//! a review that is already started), creating that review when there is none.

use super::super::github::repo_api;
use super::super::{Auth, Target, post};
use super::github::time;
use super::github_threads::{self, Login, Node, graphql, login};
use super::{NewComment, NewThread, ReviewThread, ThreadComment, new_thread_id, side_name};
use crate::diff_rows::LineSide;
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

/// The thread selection the new thread is read with: the same as the page's threads.
pub(super) const THREAD_FIELDS: &str = "id isResolved isOutdated path line originalLine startLine diffSide viewerCanReply viewerCanResolve viewerCanUnresolve resolvedBy{login} comments(first:100){totalCount nodes{databaseId author{login} body createdAt url state diffHunk}}";

const FIND: &str = "query($owner:String!,$repo:String!,$number:Int!){repository(owner:$owner,name:$repo){pullRequest(number:$number){id reviews(states:[PENDING],first:20){nodes{id databaseId viewerDidAuthor}}}}}";
const START_REVIEW: &str =
    "mutation($pr:ID!,$sha:GitObjectID!){addPullRequestReview(input:{pullRequestId:$pr,commitOID:$sha}){pullRequestReview{id databaseId}}}";

const PENDING_REVIEW: &str = "You have a review in progress on the service. Add this comment to it, or finish the review first.";
const NO_COMMENT: &str = "The service didn't answer with the comment.";
const NO_PULL: &str = "The service didn't answer with the pull request.";
const NO_REVIEW: &str = "The service didn't answer with the review.";
const NO_THREAD: &str = "The service didn't answer with the thread.";

/// A REST review comment, as a new single comment comes back.
#[derive(Deserialize)]
struct ApiNewComment {
    id: u64,
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    original_line: Option<u32>,
    #[serde(default)]
    diff_hunk: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNode {
    #[serde(default)]
    id: String,
    #[serde(default)]
    database_id: Option<u64>,
    #[serde(default)]
    viewer_did_author: bool,
}

fn side(s: Option<&str>, or: LineSide) -> LineSide {
    match s {
        Some("LEFT") => LineSide::Old,
        Some(_) => LineSide::New,
        None => or,
    }
}

/// The service's first error message, made safe to show, or `fallback`.
fn graphql_error(answer: &Value, fallback: &str) -> Error {
    Error::Other(answer["errors"][0]["message"].as_str().map_or_else(|| fallback.to_owned(), crate::text::printable))
}

/// Posts a single comment (not part of a review) on the anchor's lines.
pub(super) fn single(t: &Target, auth: &Auth, id: u64, c: &NewComment, body: &str) -> Result<NewThread> {
    let a = &c.anchor;
    let mut req = json!({ "body": body, "commit_id": c.head_sha, "path": a.path, "line": a.line, "side": side_name(a.side) });
    if let Some((line, s)) = a.start {
        req["start_line"] = json!(line);
        req["start_side"] = json!(side_name(s));
    }
    let url = format!("{}/pulls/{id}/comments", repo_api(t));
    let n: ApiNewComment = post(auth, &url, req).map_err(|e| match e {
        Error::Http { status: 422, message } if message.to_lowercase().contains("pending review") => Error::Other(PENDING_REVIEW.into()),
        Error::Decode(_) => Error::Other(NO_COMMENT.into()),
        e => e,
    })?;
    let thread = ReviewThread {
        id: new_thread_id(n.id),
        path: if n.path.is_empty() { a.path.clone() } else { n.path },
        side: side(n.side.as_deref(), a.side),
        line: n.line.or(Some(a.line)),
        start_line: n.start_line,
        original_line: n.original_line,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: false,
        can_unresolve: false,
        context: n.diff_hunk.as_deref().map(github_threads::context).unwrap_or_default(),
        comments: vec![ThreadComment {
            id: Some(n.id),
            author: login(n.user),
            time: time(&n.created_at),
            body: n.body.unwrap_or_default(),
            url: n.html_url,
            pending: false,
        }],
        more: 0,
    };
    Ok(NewThread { thread, pending_review: None })
}

/// A pending review: its node id and database id.
type Review = (String, Option<u64>);

/// The pull request's node id and the signed-in user's pending review (`pending` when it is
/// theirs, else the first of theirs) as (node id, database id).
fn find(t: &Target, auth: &Auth, id: u64, pending: Option<u64>) -> Result<(String, Option<Review>)> {
    let answer = graphql(t, auth, FIND, json!({ "owner": t.owner, "repo": t.repo, "number": id }))?;
    let pull = &answer["data"]["repository"]["pullRequest"];
    let Some(pr) = pull["id"].as_str().filter(|s| !s.is_empty()) else { return Err(graphql_error(&answer, NO_PULL)) };
    let nodes: Vec<ReviewNode> = serde_json::from_value(pull["reviews"]["nodes"].clone()).unwrap_or_default();
    let mine: Vec<&ReviewNode> = nodes.iter().filter(|n| n.viewer_did_author && !n.id.is_empty()).collect();
    let review = mine.iter().find(|n| pending.is_some() && n.database_id == pending).or(mine.first());
    Ok((pr.to_owned(), review.map(|n| (n.id.clone(), n.database_id))))
}

/// Adds the comment to the user's pending review, starting one on `head_sha` when there is
/// none.
pub(super) fn to_review(t: &Target, auth: &Auth, id: u64, pending: Option<u64>, c: &NewComment, body: &str) -> Result<NewThread> {
    let (pr, review) = find(t, auth, id, pending)?;
    let (review, database_id) = match review {
        Some(r) => r,
        None => {
            let answer = graphql(t, auth, START_REVIEW, json!({ "pr": pr, "sha": c.head_sha }))?;
            let r = &answer["data"]["addPullRequestReview"]["pullRequestReview"];
            let Some(node) = r["id"].as_str().filter(|s| !s.is_empty()) else { return Err(graphql_error(&answer, NO_REVIEW)) };
            (node.to_owned(), r["databaseId"].as_u64())
        }
    };
    let a = &c.anchor;
    let mut vars = json!({ "review": review, "path": a.path, "body": body, "line": a.line, "side": side_name(a.side) });
    if let Some((line, s)) = a.start {
        vars["startLine"] = json!(line);
        vars["startSide"] = json!(side_name(s));
    }
    let query = format!(
        "mutation($review:ID!,$path:String!,$body:String!,$line:Int!,$side:DiffSide!,$startLine:Int,$startSide:DiffSide){{addPullRequestReviewThread(input:{{pullRequestReviewId:$review,path:$path,body:$body,line:$line,side:$side,startLine:$startLine,startSide:$startSide}}){{thread{{{THREAD_FIELDS}}}}}}}"
    );
    let answer = graphql(t, auth, &query, vars)?;
    let node = &answer["data"]["addPullRequestReviewThread"]["thread"];
    let thread = if node.is_object() { serde_json::from_value::<Node>(node.clone()).ok().and_then(github_threads::thread) } else { None };
    let Some(thread) = thread else { return Err(graphql_error(&answer, NO_THREAD)) };
    Ok(NewThread { thread, pending_review: database_id })
}

#[cfg(test)]
mod tests {
    use super::super::github_threads::THREADS_QUERY;
    use super::*;

    #[test]
    fn the_new_thread_is_read_like_the_pages() {
        assert!(THREADS_QUERY.contains(THREAD_FIELDS), "the selections have drifted apart");
    }
}
