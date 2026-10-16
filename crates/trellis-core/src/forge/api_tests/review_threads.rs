use super::review::{NO_THREADS, REPO, gh, page_server_with, pull};
use super::*;
use crate::diff_rows::LineSide;
use crate::forge::review::github_threads::MAX_THREAD_PAGES;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn answer(nodes: &str, next: Option<&str>) -> String {
    let info = match next {
        Some(c) => format!(r#"{{"hasNextPage":true,"endCursor":"{c}"}}"#),
        None => r#"{"hasNextPage":false,"endCursor":null}"#.to_owned(),
    };
    format!(r#"{{"data":{{"repository":{{"pullRequest":{{"reviewThreads":{{"pageInfo":{info},"nodes":[{nodes}]}}}}}}}}}}"#)
}

fn node(id: &str, extra: &str, total: usize, comments: &str) -> String {
    format!(
        r#"{{"id":"{id}","path":"src/a.rs","diffSide":"RIGHT","viewerCanReply":true,"viewerCanResolve":true,"viewerCanUnresolve":false,
        "isResolved":false,"isOutdated":false,"line":null,"originalLine":null,"startLine":null,"resolvedBy":null,{extra}
        "comments":{{"totalCount":{total},"nodes":[{comments}]}}}}"#
    )
}

fn comment(id: u64, author: &str, at: &str, state: &str, url: &str) -> String {
    format!(
        r#"{{"databaseId":{id},"author":{author},"body":"Comment {id}","createdAt":"{at}","url":"{url}","state":"{state}",
        "diffHunk":"@@ -1,5 +1,6 @@\n a\n b\n-c\n+d\n+e\n f"}}"#
    )
}

const ADA: &str = r#"{"login":"ada"}"#;
const LINK: &str = "https://github.com/o/r/pull/7#r1";

fn threads_fixture() -> String {
    let nodes = [
        node(
            "T1",
            r#""line":12,"startLine":10,"originalLine":12,"#,
            3,
            &[
                comment(55, ADA, "2026-01-02T10:00:00Z", "SUBMITTED", LINK),
                comment(56, r#"{"login":"me"}"#, "2026-01-02T11:00:00Z", "PENDING", LINK),
            ]
            .join(","),
        ),
        node(
            "T2",
            r#""diffSide":"LEFT","line":3,"path":"src/b.rs","#,
            1,
            &comment(60, "null", "2026-01-01T10:00:00Z", "SUBMITTED", "https://evil.example/x"),
        ),
        node("T3", r#""isOutdated":true,"originalLine":7,"#, 1, &comment(61, ADA, "2026-01-03T10:00:00Z", "SUBMITTED", LINK)),
        node(
            "T4",
            r#""isResolved":true,"line":4,"resolvedBy":{"login":"grace"},"viewerCanResolve":false,"viewerCanUnresolve":true,"#,
            1,
            &comment(62, ADA, "2026-01-04T10:00:00Z", "SUBMITTED", LINK),
        ),
        node("T5", "", 1, &comment(63, ADA, "2026-01-05T10:00:00Z", "SUBMITTED", LINK)),
        node("T6", "", 0, ""),
    ];
    answer(&nodes.join(","), None)
}

fn load(graphql: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> (Mock, crate::error::Result<PrPage>) {
    let m = page_server_with("api.github.com", REPO, pull("true", "clean", "null"), graphql);
    let page = pr_page(&gh(), &token(Provider::GitHub), 7);
    (m, page)
}

fn graphql_requests(m: &Mock) -> Vec<Request> {
    m.requests().into_iter().filter(|r| r.route() == "POST api.github.com/graphql").collect()
}

#[test]
fn pr_page_reads_review_threads() {
    let body = threads_fixture();
    let (m, page) = load(move |_| Reply::json(body.clone()));
    let page = page.unwrap();
    let ids: Vec<&str> = page.threads.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["T2", "T1", "T3", "T4", "T5"], "by first comment, without the empty thread");
    assert_eq!(page.threads_note, None);

    let t1 = &page.threads[1];
    assert_eq!(
        (t1.path.as_str(), t1.side, t1.line, t1.start_line, t1.original_line),
        ("src/a.rs", LineSide::New, Some(12), Some(10), Some(12))
    );
    assert!(!t1.outdated && !t1.resolved && t1.can_reply && t1.can_resolve && !t1.can_unresolve);
    assert_eq!(t1.comments.len(), 2);
    assert_eq!(t1.more, 1, "three on the service, two read");
    assert_eq!(t1.reply_to(), Some(55));
    let c = &t1.comments[0];
    assert_eq!(
        (c.id, c.author.as_str(), c.body.as_str(), c.time, c.url.as_str(), c.pending),
        (Some(55), "ada", "Comment 55", 1_767_348_000, LINK, false)
    );
    assert!(t1.comments[1].pending, "a comment of my unsubmitted review");
    assert_eq!(t1.context, "-c\n+d\n+e\n f", "the last four lines of the hunk");
    assert_eq!(t1.time(), 1_767_348_000);

    let t2 = &page.threads[0];
    assert_eq!((t2.side, t2.line, t2.path.as_str()), (LineSide::Old, Some(3), "src/b.rs"));
    assert_eq!(t2.comments[0].author, "ghost");
    assert_eq!(t2.comments[0].url, "", "a link elsewhere is dropped");

    let t3 = &page.threads[2];
    assert!(t3.outdated);
    assert_eq!((t3.line, t3.original_line), (None, Some(7)));
    let t4 = &page.threads[3];
    assert!(t4.resolved && !t4.can_resolve && t4.can_unresolve);
    assert_eq!(t4.resolved_by.as_deref(), Some("grace"));
    let t5 = &page.threads[4];
    assert_eq!((t5.line, t5.original_line, t5.outdated), (None, None, false), "a comment on the whole file");

    let asks = graphql_requests(&m);
    assert_eq!(asks.len(), 1);
    let sent = asks[0].json();
    assert_eq!(sent["variables"], serde_json::json!({ "owner": "o", "repo": "r", "number": 7, "after": null }));
    let query = sent["query"].as_str().unwrap();
    assert!(query.contains("reviewThreads(first:50,after:$after)") && !query.contains("\"o\""), "values are variables: {query}");
    assert_eq!(asks[0].header("Authorization"), Some("Bearer tok"));
}

#[test]
fn review_threads_follow_pages() {
    let (m, page) = load(|r| {
        let one = node("T1", "", 1, &comment(1, ADA, "2026-01-02T10:00:00Z", "SUBMITTED", LINK));
        let two = node("T2", "", 1, &comment(2, ADA, "2026-01-01T10:00:00Z", "SUBMITTED", LINK));
        if r.json()["variables"]["after"].is_null() { Reply::json(answer(&one, Some("c1"))) } else { Reply::json(answer(&two, None)) }
    });
    let page = page.unwrap();
    assert_eq!(page.threads.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["T2", "T1"]);
    let asks = graphql_requests(&m);
    assert_eq!(asks.len(), 2);
    assert_eq!(asks[1].json()["variables"]["after"], "c1");
    assert_eq!(page.threads_note, None);

    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let (m, page) = load(move |_| {
        let n = seen.fetch_add(1, Ordering::SeqCst);
        Reply::json(answer(
            &node(&format!("T{n}"), "", 1, &comment(n as u64, ADA, "2026-01-02T10:00:00Z", "SUBMITTED", LINK)),
            Some("more"),
        ))
    });
    let page = page.unwrap();
    assert_eq!(graphql_requests(&m).len(), MAX_THREAD_PAGES, "a server that never stops is cut off");
    assert_eq!(page.threads.len(), MAX_THREAD_PAGES);
    assert_eq!(page.threads_note.as_deref(), Some("Only the first 1,000 code comment threads are shown."));
}

#[test]
fn graphql_errors_leave_a_note() {
    let (_m, page) =
        load(|_| Reply::json(r#"{"data":{"repository":{"pullRequest":null}},"errors":[{"message":"Field 'isOutdated' doesn't exist"}]}"#));
    let page = page.unwrap();
    assert!(page.threads.is_empty());
    assert_eq!(page.threads_note.as_deref(), Some("This server can't list code comments."));
    assert_eq!(page.detail.head_sha, "h1", "the rest of the page loaded");

    let (_m, page) = load(|r| {
        if r.json()["variables"]["after"].is_null() {
            Reply::json(answer(&node("T1", "", 1, &comment(1, ADA, "2026-01-02T10:00:00Z", "SUBMITTED", LINK)), Some("c1")))
        } else {
            Reply::json(r#"{"data":null,"errors":[{"message":"timeout"}]}"#)
        }
    });
    let page = page.unwrap();
    assert_eq!(page.threads.len(), 1, "what was read is kept");
    assert_eq!(page.threads_note.as_deref(), Some("Some code comments couldn't be loaded."));

    let (_m, page) = load(|_| Reply::status(502, "bad gateway"));
    assert!(matches!(page, Err(Error::Http { status: 502, .. })), "{page:?}");
}

#[test]
fn empty_thread_list_has_no_note() {
    let (_m, page) = load(|_| Reply::json(NO_THREADS));
    let page = page.unwrap();
    assert!(page.threads.is_empty());
    assert_eq!(page.threads_note, None);
}

#[test]
fn reply_posts_to_the_first_comment() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/pulls/7/comments/55/replies" => Reply::json(
            r#"{"id":77,"user":{"login":"me"},"body":"Done","created_at":"2026-01-02T03:04:05Z","html_url":"https://github.com/o/r/pull/7#r77"}"#,
        ),
        _ => Reply::not_found(),
    });
    let c = reply_to_thread(&gh(), &token(Provider::GitHub), 7, 55, "Done").unwrap();
    assert_eq!(find(&m, "POST").json(), serde_json::json!({ "body": "Done" }));
    let want = ThreadComment {
        id: Some(77),
        author: "me".into(),
        time: 1_767_323_045,
        body: "Done".into(),
        url: "https://github.com/o/r/pull/7#r77".into(),
        pending: false,
    };
    assert_eq!(c, want);
}

#[test]
fn reply_422_pending_review_reads_as_finish_review() {
    let call = || reply_to_thread(&gh(), &token(Provider::GitHub), 7, 55, "Done").map(drop);
    let e = error_for(
        422,
        r#"{"message":"Validation Failed","errors":[{"message":"user_id can only have one pending review per pull request"}]}"#,
        call,
    );
    assert!(matches!(&e, Error::Other(m) if m.starts_with("You have a review in progress")), "{e}");
    let e = error_for(422, r#"{"message":"Validation Failed","errors":[{"message":"body is too long"}]}"#, call);
    assert!(matches!(&e, Error::Http { status: 422, message } if message.contains("body is too long")), "{e}");
}

#[test]
fn resolve_and_unresolve_send_the_mutation() {
    let m = Mock::start(|r| {
        let q = r.json()["query"].as_str().unwrap_or_default().to_owned();
        let (field, state) =
            if q.contains("unresolveReviewThread") { ("unresolveReviewThread", false) } else { ("resolveReviewThread", true) };
        Reply::json(format!(r#"{{"data":{{"{field}":{{"thread":{{"isResolved":{state}}}}}}}}}"#))
    });
    let (t, auth) = (gh(), token(Provider::GitHub));
    assert!(resolve_thread(&t, &auth, "T_1", true).unwrap());
    assert!(!resolve_thread(&t, &auth, "T_1", false).unwrap());
    let sent: Vec<serde_json::Value> = m.requests().iter().map(|r| r.json()).collect();
    assert!(sent[0]["query"].as_str().unwrap().contains("resolveReviewThread(input:{threadId:$id})"));
    assert!(sent[1]["query"].as_str().unwrap().contains("unresolveReviewThread"));
    for s in &sent {
        assert_eq!(s["variables"], serde_json::json!({ "id": "T_1" }));
    }
    assert!(m.requests().iter().all(|r| r.route() == "POST api.github.com/graphql"));
}

#[test]
fn resolve_error_is_the_servers_message() {
    let _m = Mock::start(|_| {
        Reply::json(r#"{"data":{"resolveReviewThread":null},"errors":[{"message":"Resource not accessible\u001b by integration"}]}"#)
    });
    let e = resolve_thread(&gh(), &token(Provider::GitHub), "T_1", true).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "Resource not accessible by integration"), "{e}");
    drop(_m);
    let _m = Mock::start(|_| Reply::json(r#"{"data":{"resolveReviewThread":{"thread":null}}}"#));
    let e = resolve_thread(&gh(), &token(Provider::GitHub), "T_1", true).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "The service didn't answer with the thread."), "{e}");
}
