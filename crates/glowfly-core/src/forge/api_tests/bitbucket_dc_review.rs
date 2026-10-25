use super::*;
use serde_json::{Value, json};
use std::sync::Mutex;

const HOST: &str = "bitbucket.acme.io";
const REPO: &str = "bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/demo";
const PR: &str = "/pull-requests/3";
const WHOAMI: &str = "GET bitbucket.acme.io/plugins/servlet/applinks/whoami";
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const WEB: &str = "https://bitbucket.acme.io/projects/PROJ/repos/demo/pull-requests/3";

fn dc() -> Target {
    let t = remote("https://bitbucket.acme.io/scm/PROJ/demo.git");
    assert_eq!((t.provider, t.host.as_str()), (Provider::BitbucketDataCenter, HOST));
    t
}

fn auth() -> Auth {
    token(Provider::BitbucketDataCenter)
}

fn pr(participants: Value) -> Value {
    json!({"id":3,"version":4,"state":"OPEN","draft":true,
        "author":{"user":{"name":"ada","slug":"ada","displayName":"Ada"},"status":"UNAPPROVED"},
        "reviewers":[{"user":{"name":"linus","slug":"linus","displayName":"Linus"},"status":"APPROVED"}],
        "participants":participants,
        "fromRef":{"latestCommit":HEAD,"repository":{"id":7}},"toRef":{"latestCommit":"fedcba9876543210fedcba9876543210fedcba98","repository":{"id":7}}})
}

fn me_working() -> Value {
    json!([{"user":{"name":"Ada.L","slug":"ada.l","displayName":"Ada Lovelace"},"status":"NEEDS_WORK"}])
}

fn activities() -> Value {
    json!([
        {"action":"REVIEWED","createdDate":1767400000000i64,"user":{"displayName":"Ada Lovelace"}},
        {"action":"COMMENTED","commentAction":"ADDED","createdDate":1767360000000i64,"user":{"displayName":"Linus"},"commentAnchor":{"path":"a.rs"},
            "comment":{"id":20,"text":"On a line","author":{"displayName":"Linus"},"createdDate":1767360000000i64}},
        {"action":"APPROVED","createdDate":1767350000000i64,"user":{"displayName":"Linus"}},
        {"action":"COMMENTED","commentAction":"ADDED","createdDate":1767300000000i64,"user":{"displayName":"Ada"},
            "comment":{"id":10,"text":"Ready","author":{"displayName":"Ada"},"createdDate":1767300000000i64}},
    ])
}

const SETTINGS: &str = r#"{"mergeConfig":{"defaultStrategy":{"id":"squash"},"strategies":[{"id":"no-ff","enabled":true},
    {"id":"squash","enabled":true},{"id":"ff","enabled":true},{"id":"rebase-ff-only","enabled":false}]}}"#;

struct Answers {
    pr: Value,
    check: Reply,
    settings: Reply,
}

fn answers() -> Answers {
    Answers {
        pr: pr(me_working()),
        check: Reply::json(r#"{"canMerge":false,"conflicted":false,"outcome":"CLEAN","vetoes":[{"summaryMessage":"Needs approvals"}]}"#),
        settings: Reply::json(SETTINGS),
    }
}

fn server(a: Answers) -> Mock {
    let Answers { pr, check, settings } = a;
    let (check, settings) = (Mutex::new(Some(check)), Mutex::new(Some(settings)));
    Mock::start(move |r| {
        let route = r.route();
        if route == WHOAMI {
            return Reply::text("Ada.L\n");
        }
        let Some(path) = route.strip_prefix(&format!("GET {REPO}")) else { return Reply::not_found() };
        match path {
            PR => Reply::json(pr.to_string()),
            "/pull-requests/3/merge" => check.lock().unwrap().take().unwrap_or_else(Reply::not_found),
            "/settings/pull-requests" => settings.lock().unwrap().take().unwrap_or_else(Reply::not_found),
            "/pull-requests/3/activities?limit=100" => Reply::json(json!({"values":activities(),"isLastPage":true}).to_string()),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn pr_page_reads_the_pull_request_merge_check_settings_and_activities() {
    let m = server(answers());
    let page = pr_page(&dc(), &auth(), 3).unwrap();
    let mut want = vec![WHOAMI.to_owned()];
    let paths = [PR, "/pull-requests/3/merge", "/settings/pull-requests", "/pull-requests/3/activities?limit=100"];
    want.extend(paths.iter().map(|p| format!("GET {REPO}{p}")));
    assert_eq!(routes(&m), want);
    assert!(m.requests().iter().all(|r| r.header("Authorization") == Some("Bearer tok")));
    let d = &page.detail;
    assert_eq!((d.state, d.draft, d.locked, d.merge_state, d.mergeable), (PrState::Open, true, false, MergeState::Blocked, None));
    assert_eq!(d.head_sha, HEAD);
    assert!(d.same_repo, "the same repository id");
    assert_eq!((d.author.as_str(), d.me.as_str(), d.can_merge, d.deletes_merged_branches), ("ada", "Ada.L", None, false));
    assert_eq!(d.methods, [MergeMethod::Squash, MergeMethod::Merge], "enabled ones, the default first");
    assert_eq!(d.style, ReviewStyle::Votes { mine: Vote::ChangesRequested });
    let summary: Vec<(&str, EntryKind)> = page.entries.iter().map(|e| (e.author.as_str(), e.kind)).collect();
    assert_eq!(
        summary,
        [
            ("Ada", EntryKind::Comment),
            ("Linus", EntryKind::Review(Verdict::Approved)),
            ("Ada Lovelace", EntryKind::Review(Verdict::ChangesRequested))
        ]
    );
    assert_eq!(page.entries[0].url, format!("{WEB}/overview?commentId=10"));
    assert_eq!(
        page.threads_note.as_deref(),
        Some("Code comments on Bitbucket pull requests open in the browser for now (1 on this pull request).")
    );
}

#[test]
fn activities_are_read_by_next_page_start() {
    let m = Mock::start(|r| {
        let route = r.route();
        if route == WHOAMI {
            Reply::text("me")
        } else if route.ends_with("/activities?limit=100") {
            let first = json!([{"action":"APPROVED","createdDate":1767350000000i64,"user":{"displayName":"Linus"}}]);
            Reply::json(json!({"values":first,"isLastPage":false,"nextPageStart":1}).to_string())
        } else if route.ends_with("/activities?limit=100&start=1") {
            let second = json!([{"action":"UNAPPROVED","createdDate":1767360000000i64,"user":{"displayName":"Linus"}}]);
            Reply::json(json!({"values":second,"isLastPage":true}).to_string())
        } else if route.ends_with(PR) {
            Reply::json(pr(json!([])).to_string())
        } else {
            Reply::not_found()
        }
    });
    let page = pr_page(&dc(), &auth(), 3).unwrap();
    let kinds: Vec<EntryKind> = page.entries.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EntryKind::Review(Verdict::Approved), EntryKind::Review(Verdict::ApprovalRemoved)]);
    assert_eq!(routes(&m).iter().filter(|r| r.contains("/activities?")).count(), 2);
}

#[test]
fn unreadable_settings_fall_back_to_a_merge_commit() {
    for status in [401, 404] {
        let _m = server(Answers { settings: Reply::status(status, r#"{"errors":[{"message":"no"}]}"#), ..answers() });
        let page = pr_page(&dc(), &auth(), 3).unwrap_or_else(|e| panic!("{status}: {e}"));
        assert_eq!(page.detail.methods, [MergeMethod::Merge], "{status}");
    }
    let _m = server(Answers { check: Reply::not_found(), ..answers() });
    assert_eq!(pr_page(&dc(), &auth(), 3).unwrap().detail.merge_state, MergeState::Unknown);
}

#[test]
fn post_comment_sends_text() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/demo/pull-requests/3/comments" => {
            Reply::status(201, r#"{"id":41,"text":"Thanks!","author":{"displayName":"Ada"},"createdDate":1767348000000}"#)
        }
        _ => Reply::not_found(),
    });
    let e = post_comment(&dc(), &auth(), 3, "Thanks!").unwrap();
    assert_eq!(find(&m, "POST").json(), json!({ "text": "Thanks!" }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time), ("Ada", "Thanks!", 1_767_348_000));
    assert_eq!(e.url, format!("{WEB}/overview?commentId=41"));
}

fn review(event: ReviewEvent, body: &str) -> ReviewRequest {
    ReviewRequest { event, body: body.into(), pending: None, head_sha: HEAD.into() }
}

/// A review server: the pull request lists `participants`, the user directory answers
/// `users`, and the participant PUT answers `put_status`.
fn review_server(participants: Value, users: &'static str, put_status: u16) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        if route == WHOAMI {
            Reply::text("Ada.L")
        } else if route == format!("GET {REPO}{PR}") {
            Reply::json(pr(participants.clone()).to_string())
        } else if route.starts_with("GET bitbucket.acme.io/rest/api/1.0/users?") {
            Reply::json(users)
        } else if route.starts_with(&format!("PUT {REPO}{PR}/participants/")) {
            Reply::status(put_status, r#"{"user":{"name":"Ada.L"},"status":"APPROVED"}"#)
        } else if route == format!("POST {REPO}{PR}/comments") {
            Reply::status(201, r#"{"id":9,"text":"x"}"#)
        } else {
            Reply::not_found()
        }
    })
}

fn puts(m: &Mock) -> Vec<(String, Value)> {
    m.requests().iter().filter(|r| r.method == "PUT").map(|r| (r.path.clone(), r.json())).collect()
}

#[test]
fn submit_review_sets_the_participant_status() {
    let (t, auth) = (dc(), auth());
    let m = review_server(me_working(), r#"{"values":[]}"#, 200);
    for ev in [ReviewEvent::Approve, ReviewEvent::RequestChanges, ReviewEvent::Unapprove, ReviewEvent::UnrequestChanges] {
        submit_review(&t, &auth, 3, &review(ev, "")).unwrap_or_else(|e| panic!("{ev:?}: {e}"));
    }
    let path = "/rest/api/1.0/projects/PROJ/repos/demo/pull-requests/3/participants/ada.l";
    let body = |approved: bool, status: &str| (path.to_owned(), json!({"user":{"name":"Ada.L"},"approved":approved,"status":status}));
    assert_eq!(
        puts(&m),
        [body(true, "APPROVED"), body(false, "NEEDS_WORK"), body(false, "UNAPPROVED"), body(false, "UNAPPROVED")],
        "the slug from the pull request's participants"
    );
    assert!(!routes(&m).iter().any(|r| r.contains("/users?")), "no lookup needed");
    drop(m);

    let users = r#"{"values":[{"name":"Ada.Lx","slug":"nope"},{"name":"Ada.L","slug":"ada-l"}],"isLastPage":true}"#;
    let m = review_server(json!([]), users, 200);
    submit_review(&t, &auth, 3, &review(ReviewEvent::Approve, "Ship it")).unwrap();
    assert!(routes(&m).iter().any(|r| r.starts_with("GET bitbucket.acme.io/rest/api/1.0/users?filter=Ada.L&limit=25")));
    assert_eq!(puts(&m)[0].0, "/rest/api/1.0/projects/PROJ/repos/demo/pull-requests/3/participants/ada-l");
    let posted: Vec<Value> = m.requests().iter().filter(|r| r.method == "POST").map(|r| r.json()).collect();
    assert_eq!(posted, [json!({"text":"Ship it"})], "the summary after the vote");
    drop(m);

    let m = review_server(json!([]), r#"{"values":[{"name":"someone","slug":"someone"}]}"#, 200);
    submit_review(&t, &auth, 3, &review(ReviewEvent::Approve, "")).unwrap();
    assert_eq!(puts(&m)[0].0, "/rest/api/1.0/projects/PROJ/repos/demo/pull-requests/3/participants/Ada.L", "the name itself");
}

#[test]
fn review_refusals_keep_the_credential() {
    let _m = review_server(me_working(), r#"{"values":[]}"#, 401);
    let e = submit_review(&dc(), &auth(), 3, &review(ReviewEvent::Approve, "")).unwrap_err();
    assert!(matches!(&e, Error::Forbidden(m) if m.contains("didn't take the review")) && !e.is_auth(), "{e}");
}

fn merge_req(method: MergeMethod, title: &str, message: &str) -> MergeRequest {
    MergeRequest { method, head_sha: HEAD.into(), title: title.into(), message: message.into() }
}

/// A merge server: the fresh pull request is `fresh` and the merge answers `merge`.
fn merge_server(fresh: Value, merge: impl Fn() -> Reply + Send + Sync + 'static) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        if route == format!("GET {REPO}{PR}") {
            Reply::json(fresh.to_string())
        } else if route.starts_with(&format!("POST {REPO}{PR}/merge?")) {
            merge()
        } else {
            Reply::not_found()
        }
    })
}

fn merged() -> Reply {
    Reply::json(r#"{"id":3,"version":5,"state":"MERGED","properties":{"mergeCommit":{"id":"m1","displayId":"m1"}}}"#)
}

#[test]
fn merge_sends_the_fresh_version_and_strategy() {
    let m = merge_server(pr(json!([])), merged);
    let (t, auth) = (dc(), auth());
    assert_eq!(merge(&t, &auth, 3, &merge_req(MergeMethod::Squash, "Title", "Body")).unwrap().sha, "m1");
    merge(&t, &auth, 3, &merge_req(MergeMethod::FastForward, "Ignored", "")).unwrap();
    let sent: Vec<(String, String, Value)> = m.requests().into_iter().map(|r| (r.method.clone(), r.path.clone(), r.json())).collect();
    let api = "/rest/api/1.0/projects/PROJ/repos/demo/pull-requests/3";
    assert_eq!(
        sent,
        [
            ("GET".into(), api.into(), Value::Null),
            ("POST".into(), format!("{api}/merge?version=4"), json!({"version":4,"strategyId":"squash","message":"Title\n\nBody"})),
            ("GET".into(), api.into(), Value::Null),
            ("POST".into(), format!("{api}/merge?version=4"), json!({"version":4,"strategyId":"ff-only"})),
        ]
    );
}

#[test]
fn merge_refusals_read_plainly() {
    let call = |answer: fn() -> Reply| {
        let _m = merge_server(pr(json!([])), answer);
        merge(&dc(), &auth(), 3, &merge_req(MergeMethod::Merge, "", "")).unwrap_err()
    };
    let e = call(|| {
        Reply::status(409, r#"{"errors":[{"message":"You are attempting to modify a pull request based on out-of-date information."}]}"#)
    });
    assert!(matches!(&e, Error::Other(m) if m.starts_with("The source branch changed")), "{e}");
    let e = call(|| Reply::status(409, r#"{"errors":[{"message":"Requires 2 approvals","vetoes":[]}]}"#));
    assert!(
        matches!(&e, Error::Other(m) if m.starts_with("Bitbucket can't merge it yet (the server said") && m.contains("Requires 2 approvals")),
        "{e}"
    );
    let e = call(|| Reply::status(401, r#"{"errors":[{"message":"no"}]}"#));
    assert!(matches!(&e, Error::Forbidden(_)) && !e.is_auth(), "{e}");
    let moved = pr(json!([])).to_string().replace(HEAD, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let m = merge_server(serde_json::from_str(&moved).unwrap(), merged);
    let e = merge(&dc(), &auth(), 3, &merge_req(MergeMethod::Merge, "", "")).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("The source branch changed")), "{e}");
    assert!(!m.requests().iter().any(|r| r.method == "POST"), "nothing merged");
}
