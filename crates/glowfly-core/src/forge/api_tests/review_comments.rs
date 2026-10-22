//! Starting code comments on diff lines: a single comment through REST, and a comment in the
//! user's pending review through GraphQL (finding the review, starting one when there is none,
//! adding the thread), on GitHub and on an Enterprise Server.

use super::review::gh;
use super::*;
use crate::diff_rows::LineSide;
use serde_json::json;

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn comment(start: Option<(u32, LineSide)>) -> NewComment {
    let anchor = LineAnchor { path: "src/a.rs".into(), line: 5, side: LineSide::New, start };
    NewComment { head_sha: HEAD.into(), anchor, body: "\nRename this\n".into() }
}

const POSTED: &str = r#"{"id":901,"node_id":"PRRC_1","pull_request_review_id":44,"path":"src/a.rs","commit_id":"0123",
    "user":{"login":"me"},"body":"Rename this","created_at":"2026-01-02T03:04:05Z","html_url":"https://github.com/o/r/pull/7#discussion_r901",
    "line":5,"side":"RIGHT","start_line":null,"start_side":null,"original_line":5,
    "diff_hunk":"@@ -1,5 +1,6 @@\n a\n b\n-c\n+d\n+e\n f"}"#;

#[test]
fn single_comment_posts_line_and_side() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/pulls/7/comments" => Reply::json(POSTED),
        _ => Reply::not_found(),
    });
    let n = post_line_comment(&gh(), &token(Provider::GitHub), 7, &comment(None)).unwrap();
    let sent = find(&m, "POST");
    assert_eq!(sent.json(), json!({ "body": "Rename this", "commit_id": HEAD, "path": "src/a.rs", "line": 5, "side": "RIGHT" }));
    assert_eq!(sent.header("Authorization"), Some("Bearer tok"));
    assert_eq!(n.pending_review, None);
    let t = &n.thread;
    assert_eq!((t.id.as_str(), t.path.as_str(), t.side, t.line, t.start_line), ("new:901", "src/a.rs", LineSide::New, Some(5), None));
    assert!(t.can_reply && !t.can_resolve && !t.can_unresolve && !t.outdated && !t.resolved);
    assert_eq!(t.context, "-c\n+d\n+e\n f");
    assert_eq!(t.reply_to(), Some(901));
    let c = &t.comments[0];
    assert_eq!(
        (c.author.as_str(), c.time, c.url.as_str(), c.pending, c.body.as_str()),
        ("me", 1_767_323_045, "https://github.com/o/r/pull/7#discussion_r901", false, "Rename this")
    );
}

#[test]
fn single_comment_range_sends_start_line_and_start_side() {
    let m = Mock::start(|_| Reply::json(POSTED));
    post_line_comment(&gh(), &token(Provider::GitHub), 7, &comment(Some((3, LineSide::Old)))).unwrap();
    let sent = find(&m, "POST").json();
    assert_eq!(
        (&sent["start_line"], &sent["start_side"], &sent["line"], &sent["side"]),
        (&json!(3), &json!("LEFT"), &json!(5), &json!("RIGHT"))
    );
}

#[test]
fn single_comment_422_pending_review_reads_as_add_to_review() {
    let call = || post_line_comment(&gh(), &token(Provider::GitHub), 7, &comment(None)).map(drop);
    let e = error_for(
        422,
        r#"{"message":"Validation Failed","errors":[{"message":"user_id can only have one pending review per pull request"}]}"#,
        call,
    );
    assert!(matches!(&e, Error::Other(m) if m.starts_with("You have a review in progress")), "{e}");
    let e = error_for(
        422,
        r#"{"message":"Validation Failed","errors":[{"message":"pull_request_review_thread.line must be part of the diff"}]}"#,
        call,
    );
    assert!(e.to_string().contains("pull_request_review_thread.line must be part of the diff"), "{e}");
    let e = error_for(201, "not json", call);
    assert!(matches!(&e, Error::Other(m) if m == "The service didn't answer with the comment."), "{e}");
}

fn find_answer(reviews: &str) -> String {
    format!(r#"{{"data":{{"repository":{{"pullRequest":{{"id":"PR_node","reviews":{{"nodes":[{reviews}]}}}}}}}}}}"#)
}

const THREAD: &str = r#"{"data":{"addPullRequestReviewThread":{"thread":{"id":"PRRT_1","path":"src/a.rs","diffSide":"RIGHT",
    "line":5,"startLine":null,"originalLine":5,"isResolved":false,"isOutdated":false,"viewerCanReply":true,"viewerCanResolve":true,
    "viewerCanUnresolve":false,"resolvedBy":null,"comments":{"totalCount":1,"nodes":[{"databaseId":902,"author":{"login":"me"},
    "body":"Rename this","createdAt":"2026-01-02T03:04:05Z","url":"https://github.com/o/r/pull/7#discussion_r902","state":"PENDING",
    "diffHunk":"@@ -1 +1 @@\n+x"}]}}}}}"#;

/// A GraphQL server answering by mutation: `find` for the review lookup.
fn graphql_server(find: String, thread: &'static str) -> Mock {
    Mock::start(move |r| {
        let q = r.json()["query"].as_str().unwrap_or_default().to_owned();
        if q.contains("addPullRequestReviewThread") {
            Reply::json(thread)
        } else if q.contains("addPullRequestReview(") {
            Reply::json(r#"{"data":{"addPullRequestReview":{"pullRequestReview":{"id":"PRR_new","databaseId":31}}}}"#)
        } else {
            Reply::json(find.clone())
        }
    })
}

#[test]
fn review_comment_starts_a_review_when_none_is_pending() {
    let m = graphql_server(find_answer(""), THREAD);
    let n = add_review_comment(&gh(), &token(Provider::GitHub), 7, None, &comment(None)).unwrap();
    let asks = m.requests();
    assert!(asks.iter().all(|r| r.route() == "POST api.github.com/graphql"));
    let sent: Vec<serde_json::Value> = asks.iter().map(|r| r.json()).collect();
    assert_eq!(sent.len(), 3, "find, start the review, add the thread");
    assert!(sent[0]["query"].as_str().unwrap().contains("reviews(states:[PENDING]"));
    assert_eq!(sent[0]["variables"], json!({ "owner": "o", "repo": "r", "number": 7 }));
    assert!(sent[1]["query"].as_str().unwrap().contains("addPullRequestReview(input:{pullRequestId:$pr,commitOID:$sha})"));
    assert_eq!(sent[1]["variables"], json!({ "pr": "PR_node", "sha": HEAD }));
    assert!(sent[2]["query"].as_str().unwrap().contains("addPullRequestReviewThread"));
    assert_eq!(sent[2]["variables"], json!({ "review": "PRR_new", "path": "src/a.rs", "body": "Rename this", "line": 5, "side": "RIGHT" }));
    for s in &sent {
        let q = s["query"].as_str().unwrap();
        assert!(!q.contains("src/a.rs") && !q.contains(HEAD) && !q.contains("Rename"), "values are variables: {q}");
    }
    assert_eq!(n.pending_review, Some(31));
    assert_eq!(n.thread.id, "PRRT_1");
    assert!(n.thread.comments[0].pending);
    assert_eq!(n.thread.reply_to(), Some(902));
}

#[test]
fn review_comment_adds_to_the_pending_review() {
    let reviews = r#"{"id":"PRR_other","databaseId":8,"viewerDidAuthor":false},{"id":"PRR_mine","databaseId":9,"viewerDidAuthor":true}"#;
    let m = graphql_server(find_answer(reviews), THREAD);
    let n = add_review_comment(&gh(), &token(Provider::GitHub), 7, Some(9), &comment(Some((3, LineSide::New)))).unwrap();
    let sent: Vec<serde_json::Value> = m.requests().iter().map(|r| r.json()).collect();
    assert_eq!(sent.len(), 2, "no review is started");
    assert!(!sent.iter().any(|s| s["query"].as_str().unwrap().contains("addPullRequestReview(")));
    let vars = &sent[1]["variables"];
    assert_eq!(vars["review"], "PRR_mine");
    assert_eq!((&vars["startLine"], &vars["startSide"]), (&json!(3), &json!("RIGHT")));
    assert_eq!(n.pending_review, Some(9));
}

#[test]
fn review_comment_errors_are_the_servers_message() {
    let failed = r#"{"data":{"addPullRequestReviewThread":null},"errors":[{"message":"Path\u001b is not part of the diff"}]}"#;
    let _m = graphql_server(find_answer(r#"{"id":"PRR_mine","databaseId":9,"viewerDidAuthor":true}"#), failed);
    let e = add_review_comment(&gh(), &token(Provider::GitHub), 7, Some(9), &comment(None)).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "Path is not part of the diff"), "{e}");
    drop(_m);

    let _m = Mock::start(|_| {
        Reply::json(r#"{"data":{"repository":{"pullRequest":null}},"errors":[{"message":"Could not resolve to a PullRequest"}]}"#)
    });
    let e = add_review_comment(&gh(), &token(Provider::GitHub), 7, None, &comment(None)).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "Could not resolve to a PullRequest"), "{e}");
    drop(_m);

    let empty = r#"{"data":{"addPullRequestReviewThread":{"thread":{"id":"PRRT_1","comments":{"totalCount":0,"nodes":[]}}}}}"#;
    let _m = graphql_server(find_answer(r#"{"id":"PRR_mine","databaseId":9,"viewerDidAuthor":true}"#), empty);
    let e = add_review_comment(&gh(), &token(Provider::GitHub), 7, Some(9), &comment(None)).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "The service didn't answer with the thread."), "{e}");
}

#[test]
fn github_enterprise_comments_use_its_api() {
    let t = Target {
        provider: Provider::GitHubEnterprise,
        host: "git.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    let auth = token(Provider::GitHubEnterprise);
    let m = Mock::start(|r| match r.route().as_str() {
        "POST git.acme.io/api/v3/repos/o/r/pulls/7/comments" => Reply::json(POSTED),
        _ => Reply::not_found(),
    });
    post_line_comment(&t, &auth, 7, &comment(None)).unwrap();
    assert_eq!(m.requests().len(), 1);
    drop(m);
    let m = graphql_server(find_answer(r#"{"id":"PRR_mine","databaseId":9,"viewerDidAuthor":true}"#), THREAD);
    add_review_comment(&t, &auth, 7, Some(9), &comment(None)).unwrap();
    assert!(m.requests().iter().all(|r| r.route() == "POST git.acme.io/api/graphql"), "{:?}", m.requests());
}
