//! Code comment threads on GitHub and GitHub Enterprise Server: read through GraphQL (REST has
//! no threads, nor whether they are resolved or outdated), replies through REST, and resolving
//! through GraphQL mutations.

use super::super::github::{graphql_url, repo_api};
use super::super::{Auth, Target, post};
use super::github::time;
use super::{ReviewThread, ThreadComment};
use crate::diff_rows::LineSide;
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

/// How many pages of threads (50 each) are read at most.
pub(in super::super) const MAX_THREAD_PAGES: usize = 20;

/// The lines of the first comment's diff kept as the thread's snippet.
const CONTEXT_LINES: usize = 4;

pub(super) const THREADS_QUERY: &str = "query($owner:String!,$repo:String!,$number:Int!,$after:String){repository(owner:$owner,name:$repo){pullRequest(number:$number){reviewThreads(first:50,after:$after){pageInfo{hasNextPage endCursor} nodes{id isResolved isOutdated path line originalLine startLine diffSide viewerCanReply viewerCanResolve viewerCanUnresolve resolvedBy{login} comments(first:100){totalCount nodes{databaseId author{login} body createdAt url state diffHunk}}}}}}}";

const RESOLVE: &str = "mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){thread{isResolved}}}";
const UNRESOLVE: &str = "mutation($id:ID!){unresolveReviewThread(input:{threadId:$id}){thread{isResolved}}}";

const CANT_LIST: &str = "This server can't list code comments.";
const SOME_FAILED: &str = "Some code comments couldn't be loaded.";
const CAPPED: &str = "Only the first 1,000 code comment threads are shown.";
const NO_COMMENT: &str = "The service didn't answer with the comment.";
const NO_THREAD: &str = "The service didn't answer with the thread.";
const PENDING_REVIEW: &str = "You have a review in progress on the service. Finish it to reply.";

/// How a deleted account is shown.
const GHOST: &str = "ghost";

#[derive(Deserialize)]
pub(super) struct Login {
    login: String,
}

pub(super) fn login(user: Option<Login>) -> String {
    user.map_or_else(|| GHOST.to_owned(), |u| u.login)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Node {
    #[serde(default)]
    id: String,
    #[serde(default)]
    is_resolved: bool,
    #[serde(default)]
    is_outdated: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    original_line: Option<u32>,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    diff_side: Option<String>,
    #[serde(default)]
    viewer_can_reply: bool,
    #[serde(default)]
    viewer_can_resolve: bool,
    #[serde(default)]
    viewer_can_unresolve: bool,
    #[serde(default)]
    resolved_by: Option<Login>,
    #[serde(default)]
    comments: Option<Comments>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Comments {
    #[serde(default)]
    total_count: usize,
    #[serde(default)]
    nodes: Vec<CommentNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentNode {
    #[serde(default)]
    database_id: Option<u64>,
    #[serde(default)]
    author: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    diff_hunk: Option<String>,
}

/// A REST review comment (the answer to a reply).
#[derive(Deserialize)]
struct ApiReviewComment {
    id: u64,
    #[serde(default)]
    user: Option<Login>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    html_url: String,
}

pub(super) fn graphql(t: &Target, auth: &Auth, query: &str, vars: Value) -> Result<Value> {
    post(auth, &graphql_url(t), json!({ "query": query, "variables": vars }))
}

/// A snippet line: its diff marker and indentation kept, the rest made safe to show.
fn snippet_line(l: &str) -> String {
    let cut = l.find(|c: char| !matches!(c, ' ' | '\t' | '+' | '-')).unwrap_or(l.len());
    format!("{}{}", l[..cut].replace('\t', "    "), crate::text::printable(&l[cut..]))
}

/// The last few lines of a diff hunk, as plain text.
pub(super) fn context(hunk: &str) -> String {
    let lines: Vec<&str> = hunk.lines().collect();
    lines[lines.len().saturating_sub(CONTEXT_LINES)..].iter().map(|l| snippet_line(l)).collect::<Vec<_>>().join("\n")
}

fn comment(c: CommentNode) -> ThreadComment {
    ThreadComment {
        id: c.database_id,
        author: login(c.author),
        time: time(&c.created_at),
        body: c.body.unwrap_or_default(),
        url: c.url,
        pending: c.state == "PENDING",
    }
}

/// A thread from its GraphQL node; `None` when it has no comments.
pub(super) fn thread(n: Node) -> Option<ReviewThread> {
    let comments = n.comments.unwrap_or(Comments { total_count: 0, nodes: Vec::new() });
    let more = comments.total_count.saturating_sub(comments.nodes.len());
    let context = comments.nodes.first().and_then(|c| c.diff_hunk.as_deref()).map(context).unwrap_or_default();
    let list: Vec<ThreadComment> = comments.nodes.into_iter().map(comment).collect();
    if list.is_empty() {
        return None;
    }
    Some(ReviewThread {
        id: n.id,
        path: n.path,
        side: if n.diff_side.as_deref() == Some("LEFT") { LineSide::Old } else { LineSide::New },
        line: n.line,
        start_line: n.start_line,
        original_line: n.original_line,
        outdated: n.is_outdated,
        resolved: n.is_resolved,
        resolved_by: n.resolved_by.map(|l| l.login),
        can_reply: n.viewer_can_reply,
        can_resolve: n.viewer_can_resolve,
        can_unresolve: n.viewer_can_unresolve,
        context,
        comments: list,
        more,
    })
}

/// The pull request's threads ordered by their first comment, and a note when some or all of
/// them couldn't be read. A server whose GraphQL schema lacks the fields (an older Enterprise
/// Server) gives a note instead of an error, so the rest of the page still loads; HTTP and
/// credential failures are errors like those of the REST calls.
pub(super) fn threads(t: &Target, auth: &Auth, id: u64) -> Result<(Vec<ReviewThread>, Option<String>)> {
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    let mut note = None;
    for page in 0.. {
        if page == MAX_THREAD_PAGES {
            note = Some(CAPPED.to_owned());
            break;
        }
        let vars = json!({ "owner": t.owner, "repo": t.repo, "number": id, "after": after });
        let answer = graphql(t, auth, THREADS_QUERY, vars)?;
        let list = &answer["data"]["repository"]["pullRequest"]["reviewThreads"];
        if list.is_null() {
            let failed = if page == 0 { CANT_LIST } else { SOME_FAILED };
            note = Some(failed.to_owned());
            break;
        }
        let nodes: Vec<Node> = serde_json::from_value(list["nodes"].clone()).map_err(|e| Error::Decode(e.to_string()))?;
        out.extend(nodes.into_iter().filter_map(thread));
        let next = list["pageInfo"]["endCursor"].as_str().filter(|_| list["pageInfo"]["hasNextPage"].as_bool() == Some(true));
        match next {
            Some(c) => after = Some(c.to_owned()),
            None => break,
        }
    }
    out.sort_by_key(ReviewThread::time);
    Ok((out, note))
}

pub(super) fn reply(t: &Target, auth: &Auth, id: u64, comment: u64, body: &str) -> Result<ThreadComment> {
    let url = format!("{}/pulls/{id}/comments/{comment}/replies", repo_api(t));
    let c: ApiReviewComment = post(auth, &url, json!({ "body": body })).map_err(|e| match e {
        Error::Http { status: 422, message } if message.to_lowercase().contains("pending review") => Error::Other(PENDING_REVIEW.into()),
        Error::Decode(_) => Error::Other(NO_COMMENT.into()),
        e => e,
    })?;
    Ok(ThreadComment {
        id: Some(c.id),
        author: login(c.user),
        time: time(&c.created_at),
        body: c.body.unwrap_or_default(),
        url: c.html_url,
        pending: false,
    })
}

pub(super) fn resolve(t: &Target, auth: &Auth, thread: &str, resolve: bool) -> Result<bool> {
    let (query, field) = if resolve { (RESOLVE, "resolveReviewThread") } else { (UNRESOLVE, "unresolveReviewThread") };
    let answer = graphql(t, auth, query, json!({ "id": thread }))?;
    let done = &answer["data"][field];
    if done.is_null()
        && let Some(m) = answer["errors"][0]["message"].as_str()
    {
        return Err(Error::Other(crate::text::printable(m)));
    }
    done["thread"]["isResolved"].as_bool().ok_or_else(|| Error::Other(NO_THREAD.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_keeps_the_last_lines_and_their_indentation() {
        let hunk = "@@ -1,5 +1,6 @@\n fn a() {\n-\told();\n+    new();\n+    more(\u{1b}[31m);\n }";
        assert_eq!(context(hunk), "-    old();\n+    new();\n+    more([31m);\n }");
        assert_eq!(context("@@ -1 +1 @@\n+x"), "@@ -1 +1 @@\n+x", "a short hunk is kept whole");
    }
}
