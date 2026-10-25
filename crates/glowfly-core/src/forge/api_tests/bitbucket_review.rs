use super::*;
use serde_json::{Value, json};

pub(super) const API: &str = "api.bitbucket.org/2.0/repositories/team/app";
pub(super) const PR: &str = "/pullrequests/12";
const COMMENTS: &str = "/pullrequests/12/comments?pagelen=100&sort=created_on";
const ACTIVITY: &str = "/pullrequests/12/activity?pagelen=50";
const DIFFSTAT: &str = "/diffstat/team/app:0123456789ab%0Dfedcba987654?from_pullrequest_id=12&topic=true";
pub(super) const FULL: &str = "0123456789abcdef0123456789abcdef01234567";

pub(super) fn bb() -> Target {
    remote("https://bitbucket.org/team/app")
}

/// An app password with its username: Basic auth.
pub(super) fn basic() -> Auth {
    Auth::from_token(Provider::Bitbucket, Some("ada"), "app-pass", "test")
}

/// `base` with `patch`'s fields set over it.
fn patched(mut base: Value, patch: Value) -> Value {
    if let (Some(b), Value::Object(p)) = (base.as_object_mut(), patch) {
        b.extend(p);
    }
    base
}

pub(super) fn pr(patch: Value) -> Value {
    let base = json!({"id":12,"state":"OPEN","draft":true,"close_source_branch":true,
        "author":{"display_name":"Ada","uuid":"{ada}"},
        "participants":[{"user":{"display_name":"Me","uuid":"{ME}"},"role":"REVIEWER","approved":true,"state":"approved"},
            {"user":{"display_name":"Linus","uuid":"{linus}"},"role":"REVIEWER","approved":false,"state":"changes_requested"}],
        "source":{"branch":{"name":"feature"},"commit":{"hash":"0123456789ab"},"repository":{"full_name":"team/app"}},
        "destination":{"branch":{"name":"main","merge_strategies":["merge_commit","squash","fast_forward"],"default_merge_strategy":"squash"},
            "commit":{"hash":"fedcba987654"},"repository":{"full_name":"Team/App"}},
        "links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/12"},
            "diffstat":{"href":format!("https://{API}{DIFFSTAT}")}}});
    patched(base, patch)
}

fn comment(id: u64, author: &str, day: u32, body: &str, extra: Value) -> Value {
    let base = json!({"id":id,"content":{"raw":body},"user":{"display_name":author},
        "created_on":format!("2026-01-{day:02}T10:00:00.000000+00:00"),
        "links":{"html":{"href":format!("https://bitbucket.org/team/app/pull-requests/12/_/diff#comment-{id}")}}});
    patched(base, extra)
}

fn page(values: Value) -> String {
    json!({ "values": values }).to_string()
}

pub(super) struct Answers {
    pub pr: Value,
    pub commit: (u16, String),
    pub diffstat: (u16, String),
    pub comments: Value,
    pub activity: Value,
}

pub(super) fn answers() -> Answers {
    Answers {
        pr: pr(json!({})),
        commit: (200, json!({ "hash": FULL }).to_string()),
        diffstat: (200, page(json!([{"status":"modified"},{"status":"added"}]))),
        comments: json!([
            comment(1, "Ada", 1, "Ready for a look", json!({})),
            comment(2, "Grace", 2, "On this line", json!({"inline":{"path":"src/a.rs","to":4}})),
            comment(3, "Grace", 2, "Gone", json!({"deleted":true})),
            comment(4, "Linus", 4, "Will do", json!({"parent":{"id":1}})),
        ]),
        activity: json!([
            {"approval":{"date":"2026-01-02T12:00:00Z","user":{"display_name":"Grace"}}},
            {"comment":{"id":1}},
            {"changes_requested":{"date":"2026-01-03T12:00:00Z","user":{"display_name":"Linus"}}},
            {"update":{"state":"OPEN"}},
        ]),
    }
}

pub(super) fn server(a: Answers) -> Mock {
    let Answers { pr, commit, diffstat, comments, activity } = a;
    Mock::start(move |r| {
        let route = r.route();
        if route == "GET api.bitbucket.org/2.0/user" {
            return Reply::json(r#"{"display_name":"Me","uuid":"{me}"}"#);
        }
        let Some(path) = route.strip_prefix(&format!("GET {API}")) else { return Reply::not_found() };
        match path {
            PR => Reply::json(pr.to_string()),
            "/commit/0123456789ab?fields=hash" => Reply::status(commit.0, commit.1.clone()),
            p if p == format!("{DIFFSTAT}&pagelen=100") => Reply::status(diffstat.0, diffstat.1.clone()),
            COMMENTS => Reply::json(page(comments.clone())),
            ACTIVITY => Reply::json(page(activity.clone())),
            _ => Reply::not_found(),
        }
    })
}

fn page_of(a: Answers) -> (PrPage, Vec<String>) {
    let m = server(a);
    let page = pr_page(&bb(), &basic(), 12).unwrap();
    (page, routes(&m))
}

#[test]
fn pr_page_reads_the_pull_request_its_comments_and_votes() {
    let m = server(answers());
    let page = pr_page(&bb(), &basic(), 12).unwrap();
    let mut want = vec!["GET api.bitbucket.org/2.0/user".to_owned()];
    let paths = [PR, "/commit/0123456789ab?fields=hash", &format!("{DIFFSTAT}&pagelen=100"), COMMENTS, ACTIVITY];
    want.extend(paths.iter().map(|p| format!("GET {API}{p}")));
    assert_eq!(routes(&m), want);
    assert!(m.requests().iter().all(|r| r.header("Authorization").is_some_and(|h| h.starts_with("Basic "))));
    let d = &page.detail;
    assert_eq!((d.state, d.draft, d.locked, d.merge_state, d.mergeable), (PrState::Open, true, false, MergeState::Clean, Some(true)));
    assert_eq!((d.head_sha.as_str(), d.base_sha.as_str()), (FULL, "fedcba987654"));
    assert!(d.same_repo, "the full names match without case");
    assert_eq!((d.author.as_str(), d.me.as_str(), d.can_merge), ("{ada}", "{me}", None));
    assert_eq!(d.methods, [MergeMethod::Squash, MergeMethod::Merge, MergeMethod::FastForward]);
    assert!(d.deletes_merged_branches, "close_source_branch");
    assert_eq!(d.style, ReviewStyle::Votes { mine: Vote::Approved });
    let summary: Vec<(&str, EntryKind, &str)> = page.entries.iter().map(|e| (e.author.as_str(), e.kind, e.body.as_str())).collect();
    assert_eq!(
        summary,
        [
            ("Ada", EntryKind::Comment, "Ready for a look"),
            ("Grace", EntryKind::Review(Verdict::Approved), ""),
            ("Linus", EntryKind::Review(Verdict::ChangesRequested), ""),
            ("Linus", EntryKind::Comment, "Will do"),
        ],
        "oldest first, without deleted or code comments"
    );
    assert_eq!(page.entries[0].url, "https://bitbucket.org/team/app/pull-requests/12/_/diff#comment-1");
    assert_eq!(page.entries[1].url, "https://bitbucket.org/team/app/pull-requests/12");
    assert!(page.threads.is_empty());
    let n = page.threads_note.as_deref().unwrap_or_default();
    assert!(n.starts_with("Code comments on Bitbucket pull requests open in the browser") && n.contains("(1 on this pull request)"), "{n}");
}

#[test]
fn a_deleted_fork_and_a_missing_commit_keep_the_prefix() {
    let fork = json!({"source":{"branch":{"name":"feature"},"commit":{"hash":"0123456789ab"},"repository":null}});
    let (page, routes) = page_of(Answers { pr: pr(fork), ..answers() });
    assert!(!page.detail.same_repo);
    assert_eq!(page.detail.head_sha, "0123456789ab");
    assert!(!routes.iter().any(|r| r.contains("/commit/")), "{routes:?}");
    let (page, _) = page_of(Answers { commit: (404, r#"{"type":"error","error":{"message":"gone"}}"#.into()), ..answers() });
    assert_eq!(page.detail.head_sha, "0123456789ab");
    let _m = server(Answers { commit: (500, "{}".into()), ..answers() });
    assert!(pr_page(&bb(), &basic(), 12).is_err(), "other failures aren't hidden");
}

#[test]
fn conflicts_come_from_the_diffstat() {
    let conflict = (200, page(json!([{"status":"modified"},{"status":"merge conflict"}])));
    let (page, _) = page_of(Answers { diffstat: conflict, ..answers() });
    assert_eq!((page.detail.merge_state, page.detail.mergeable), (MergeState::Conflicts, Some(false)));
    let (page, _) = page_of(Answers { diffstat: (403, "{}".into()), ..answers() });
    assert_eq!((page.detail.merge_state, page.detail.mergeable), (MergeState::Unknown, None));
    let elsewhere = json!({"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/12"},
        "diffstat":{"href":"https://evil.example/diffstat"}}});
    let (page, routes) = page_of(Answers { pr: pr(elsewhere), ..answers() });
    assert_eq!(page.detail.merge_state, MergeState::Unknown);
    assert!(!routes.iter().any(|r| r.contains("diffstat")), "{routes:?}");
    let (page, routes) = page_of(Answers { pr: pr(json!({"state":"DECLINED"})), ..answers() });
    assert_eq!(page.detail.state, PrState::Closed);
    assert!(!routes.iter().any(|r| r.contains("diffstat")), "a closed pull request needs no conflicts: {routes:?}");
}

#[test]
fn comments_and_activity_follow_next_links() {
    let m = Mock::start(|r| {
        let route = r.route();
        let next = |path: &str| json!({"values":[], "next": format!("https://{API}{path}&page=2")});
        let with = |mut v: Value, values: Value| {
            v["values"] = values;
            Reply::json(v.to_string())
        };
        if route == "GET api.bitbucket.org/2.0/user" {
            Reply::json(r#"{"uuid":"{me}"}"#)
        } else if route.ends_with(&format!("{COMMENTS}&page=2")) {
            Reply::json(page(json!([comment(2, "Grace", 3, "second", json!({}))])))
        } else if route.ends_with(COMMENTS) {
            with(next(COMMENTS), json!([comment(1, "Ada", 1, "first", json!({}))]))
        } else if route.ends_with(&format!("{ACTIVITY}&page=2")) {
            Reply::json(page(json!([{"changes_requested":{"date":"2026-01-04T00:00:00Z","user":{"display_name":"Linus"}}}])))
        } else if route.ends_with(ACTIVITY) {
            with(next(ACTIVITY), json!([{"approval":{"date":"2026-01-02T00:00:00Z","user":{"display_name":"Grace"}}}]))
        } else if route.ends_with(PR) {
            Reply::json(pr(json!({"links":{}})).to_string())
        } else {
            Reply::json(json!({ "hash": FULL }).to_string())
        }
    });
    let page = pr_page(&bb(), &basic(), 12).unwrap();
    let bodies: Vec<(&str, EntryKind)> = page.entries.iter().map(|e| (e.author.as_str(), e.kind)).collect();
    assert_eq!(
        bodies,
        [
            ("Ada", EntryKind::Comment),
            ("Grace", EntryKind::Review(Verdict::Approved)),
            ("Grace", EntryKind::Comment),
            ("Linus", EntryKind::Review(Verdict::ChangesRequested)),
        ]
    );
    assert_eq!(page.threads_note, None, "no code comments, no note");
    let routes = routes(&m);
    assert_eq!(routes.iter().filter(|r| r.contains("/comments?")).count(), 2);
    assert_eq!(routes.iter().filter(|r| r.contains("/activity?")).count(), 2);
}

#[test]
fn post_comment_posts_raw_content() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.bitbucket.org/2.0/repositories/team/app/pullrequests/12/comments" => {
            Reply::status(201, comment(41, "Me", 2, "Thanks!", json!({})).to_string())
        }
        _ => Reply::not_found(),
    });
    let e = post_comment(&bb(), &basic(), 12, "Thanks!").unwrap();
    assert_eq!(find(&m, "POST").json(), json!({ "content": { "raw": "Thanks!" } }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.kind, e.time), ("Me", "Thanks!", EntryKind::Comment, 1_767_348_000));
    assert_eq!(e.url, "https://bitbucket.org/team/app/pull-requests/12/_/diff#comment-41");
}

fn review(event: ReviewEvent, body: &str) -> ReviewRequest {
    ReviewRequest { event, body: body.into(), pending: None, head_sha: FULL.into() }
}

/// A server that answers `failing` (a method and route suffix after the pull request) with
/// `status`.
fn review_server(failing: &'static str, status: u16) -> Mock {
    Mock::start(move |r| {
        let Some(rest) = r.route().strip_prefix(&format!("{} {API}{PR}", r.method)).map(str::to_owned) else {
            return Reply::not_found();
        };
        if format!("{} {rest}", r.method) == failing {
            return Reply::status(status, r#"{"type":"error","error":{"message":"refused"}}"#);
        }
        match (r.method.as_str(), rest.as_str()) {
            ("POST", "/comments") => Reply::status(201, comment(9, "Me", 1, "x", json!({})).to_string()),
            ("POST", "/approve" | "/request-changes") => Reply::json(r#"{"approved":true}"#),
            ("DELETE", "/approve" | "/request-changes") => Reply::status(204, ""),
            _ => Reply::not_found(),
        }
    })
}

fn sent(m: &Mock) -> Vec<(String, Value)> {
    m.requests().iter().map(|r| (r.route(), r.json())).collect()
}

#[test]
fn submit_review_votes_and_takes_back() {
    let (t, auth) = (bb(), basic());
    let m = review_server("", 0);
    for (ev, body) in [
        (ReviewEvent::Comment, "Nice"),
        (ReviewEvent::Approve, ""),
        (ReviewEvent::Approve, "Ship it"),
        (ReviewEvent::RequestChanges, ""),
        (ReviewEvent::Unapprove, ""),
        (ReviewEvent::UnrequestChanges, ""),
    ] {
        submit_review(&t, &auth, 12, &review(ev, body)).unwrap_or_else(|e| panic!("{ev:?}: {e}"));
    }
    let call = |method: &str, path: &str, body: Value| (format!("{method} {API}{PR}{path}"), body);
    assert_eq!(
        sent(&m),
        [
            call("POST", "/comments", json!({"content":{"raw":"Nice"}})),
            call("POST", "/approve", json!({})),
            call("POST", "/approve", json!({})),
            call("POST", "/comments", json!({"content":{"raw":"Ship it"}})),
            call("POST", "/request-changes", json!({})),
            call("DELETE", "/approve", Value::Null),
            call("DELETE", "/request-changes", Value::Null),
        ]
    );
}

#[test]
fn vote_refusals_read_plainly() {
    let (t, auth) = (bb(), basic());
    let call = |failing, status, ev, body: &str| {
        let _m = review_server(failing, status);
        submit_review(&t, &auth, 12, &review(ev, body)).unwrap_err()
    };
    let e = call("DELETE /approve", 404, ReviewEvent::Unapprove, "");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("You haven't approved this pull request")), "{e}");
    let e = call("DELETE /request-changes", 404, ReviewEvent::UnrequestChanges, "");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("You haven't requested changes")), "{e}");
    let e = call("POST /comments", 500, ReviewEvent::Approve, "Ship it");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Approved, but the summary wasn't posted")), "{e}");
    let e = call("POST /comments", 500, ReviewEvent::UnrequestChanges, "Fixed");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Removed your change request, but")), "{e}");
    let e = call("POST /approve", 401, ReviewEvent::Approve, "");
    assert!(e.is_auth(), "Cloud's 401 is bad credentials: {e}");
}

#[test]
fn other_services_dont_take_back_change_requests() {
    let m = Mock::start(|_| Reply::json("{}"));
    for t in [remote("https://github.com/o/r"), remote("https://gitlab.com/g/app")] {
        let e = submit_review(&t, &token(t.provider), 7, &review(ReviewEvent::UnrequestChanges, "")).unwrap_err();
        assert!(matches!(&e, Error::Unsupported(_)), "{:?}: {e}", t.provider);
    }
    assert!(m.requests().is_empty());
}
