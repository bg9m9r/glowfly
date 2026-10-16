use super::*;

pub(super) const REPO: &str = r#"{"allow_merge_commit":false,"allow_squash_merge":true,"allow_rebase_merge":true,
    "delete_branch_on_merge":true,"permissions":{"push":false}}"#;

pub(super) fn pull(mergeable: &str, state: &str, head_repo: &str) -> String {
    format!(
        r#"{{"state":"open","merged":false,"draft":false,"locked":true,"mergeable":{mergeable},"mergeable_state":"{state}",
        "user":{{"login":"ada"}},"head":{{"sha":"h1","repo":{head_repo}}},"base":{{"sha":"b1","repo":{{"full_name":"o/r"}}}}}}"#
    )
}

const COMMENTS: &str = r#"[
    {"user":{"login":"grace"},"body":"Looks good","created_at":"2026-01-02T10:00:00Z","html_url":"https://github.com/o/r/pull/7#c1"},
    {"user":null,"body":null,"created_at":"2026-01-01T10:00:00Z","html_url":"https://evil.example/x"}]"#;

const REVIEWS: &str = r#"[
    {"id":1,"user":{"login":"linus"},"body":"Fix the name","state":"CHANGES_REQUESTED","submitted_at":"2026-01-03T10:00:00Z","html_url":""},
    {"id":2,"user":{"login":"me"},"body":"","state":"PENDING","html_url":""},
    {"id":3,"user":{"login":"eve"},"body":"","state":"PENDING","html_url":""},
    {"id":4,"user":{"login":"grace"},"body":"","state":"APPROVED","submitted_at":"2026-01-01T12:00:00Z","html_url":""}]"#;

pub(super) fn gh() -> Target {
    remote("https://github.com/o/r")
}

/// An answer with no code comment threads.
pub(super) const NO_THREADS: &str =
    r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}}}}"#;

fn page_server(api: &'static str, repo: &'static str, pull: String) -> Mock {
    page_server_with(api, repo, pull, |_| Reply::json(NO_THREADS))
}

/// The page's REST answers under `api`, and `graphql` for the GraphQL calls (to the endpoint of
/// either GitHub or an Enterprise Server).
pub(super) fn page_server_with(
    api: &'static str,
    repo: &'static str,
    pull: String,
    graphql: impl Fn(&Request) -> Reply + Send + Sync + 'static,
) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        if route == "POST api.github.com/graphql" || route == "POST git.acme.io/api/graphql" {
            return graphql(r);
        }
        let Some(path) = route.strip_prefix(&format!("GET {api}")) else { return Reply::not_found() };
        match path {
            "/user" => Reply::json(r#"{"login":"me"}"#),
            "/repos/o/r" => Reply::json(repo),
            "/repos/o/r/pulls/7" => Reply::json(pull.clone()),
            "/repos/o/r/issues/7/comments?per_page=100" => Reply::json(COMMENTS),
            "/repos/o/r/pulls/7/reviews?per_page=100" => Reply::json(REVIEWS),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn pr_page_reads_detail_settings_comments_and_reviews() {
    let m = page_server("api.github.com", REPO, pull("true", "clean", r#"{"full_name":"O/R"}"#));
    let page = pr_page(&gh(), &token(Provider::GitHub), 7).unwrap();
    for route in [
        "GET api.github.com/user",
        "GET api.github.com/repos/o/r",
        "GET api.github.com/repos/o/r/pulls/7",
        "GET api.github.com/repos/o/r/issues/7/comments?per_page=100",
        "GET api.github.com/repos/o/r/pulls/7/reviews?per_page=100",
        "POST api.github.com/graphql",
    ] {
        assert!(routes(&m).iter().any(|r| r == route), "{route}: {:?}", routes(&m));
    }
    assert!(m.requests().iter().all(|r| r.header("Authorization") == Some("Bearer tok")));
    let d = &page.detail;
    assert_eq!((d.state, d.draft, d.locked, d.merge_state), (PrState::Open, false, true, MergeState::Clean));
    assert_eq!((d.head_sha.as_str(), d.base_sha.as_str(), d.author.as_str(), d.me.as_str()), ("h1", "b1", "ada", "me"));
    assert_eq!(d.methods, [MergeMethod::Squash, MergeMethod::Rebase]);
    assert!(d.deletes_merged_branches);
    assert_eq!(d.can_merge, Some(false));
    assert!(d.same_repo, "same repository, whatever the case");
    assert_eq!(d.pending_review, Some(2), "only my own pending review");
    let summary: Vec<(&str, EntryKind)> = page.entries.iter().map(|e| (e.author.as_str(), e.kind)).collect();
    assert_eq!(
        summary,
        [
            ("ghost", EntryKind::Comment),
            ("grace", EntryKind::Review(Verdict::Approved)),
            ("grace", EntryKind::Comment),
            ("linus", EntryKind::Review(Verdict::ChangesRequested)),
        ],
        "oldest first, without pending reviews"
    );
    assert_eq!(page.entries[0].url, "", "a link elsewhere is dropped");
    assert_eq!(page.entries[0].body, "");
    assert_eq!(page.entries[2].url, "https://github.com/o/r/pull/7#c1");
    assert_eq!(page.entries[3].body, "Fix the name");
}

#[test]
fn enterprise_uses_the_api_v3_prefix() {
    let m = page_server("git.acme.io/api/v3", REPO, pull("true", "clean", "null"));
    let t = Target {
        provider: Provider::GitHubEnterprise,
        host: "git.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    pr_page(&t, &token(Provider::GitHubEnterprise), 7).unwrap();
    assert_eq!(routes(&m).len(), 6);
    let v3 = routes(&m).iter().filter(|r| r.starts_with("GET git.acme.io/api/v3/")).count();
    assert_eq!(v3, 5, "{:?}", routes(&m));
    assert!(routes(&m).iter().any(|r| r == "POST git.acme.io/api/graphql"), "{:?}", routes(&m));
}

#[test]
fn absent_merge_settings_allow_every_method() {
    let _m = page_server("api.github.com", "{}", pull("null", "unknown", "null"));
    let d = pr_page(&gh(), &token(Provider::GitHub), 7).unwrap().detail;
    assert_eq!(d.methods, MergeMethod::ALL);
    assert!(!d.deletes_merged_branches);
    assert_eq!(d.can_merge, None);
}

#[test]
fn fork_head_repo_and_deleted_fork_are_not_same_repo() {
    let _m = page_server("api.github.com", REPO, pull("true", "clean", r#"{"full_name":"ada/r"}"#));
    assert!(!pr_page(&gh(), &token(Provider::GitHub), 7).unwrap().detail.same_repo);
    drop(_m);
    let _m = page_server("api.github.com", REPO, pull("true", "clean", "null"));
    assert!(!pr_page(&gh(), &token(Provider::GitHub), 7).unwrap().detail.same_repo, "the fork was deleted");
}

#[test]
fn mergeable_false_is_conflicts_and_null_is_unknown() {
    let _m = page_server("api.github.com", REPO, pull("false", "unknown", "null"));
    let d = pr_page(&gh(), &token(Provider::GitHub), 7).unwrap().detail;
    assert_eq!((d.mergeable, d.merge_state), (Some(false), MergeState::Conflicts));
    drop(_m);
    let _m = page_server("api.github.com", REPO, pull("null", "unknown", "null"));
    let d = pr_page(&gh(), &token(Provider::GitHub), 7).unwrap().detail;
    assert_eq!((d.mergeable, d.merge_state), (None, MergeState::Unknown));
}

#[test]
fn post_comment_posts_the_body_and_returns_the_entry() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/issues/7/comments" => Reply::json(
            r#"{"user":{"login":"me"},"body":"Thanks!","created_at":"2026-01-02T03:04:05Z","html_url":"https://github.com/o/r/pull/7#c9"}"#,
        ),
        _ => Reply::not_found(),
    });
    let e = post_comment(&gh(), &token(Provider::GitHub), 7, "Thanks!").unwrap();
    assert_eq!(find(&m, "POST").json(), serde_json::json!({ "body": "Thanks!" }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.kind, e.time), ("me", "Thanks!", EntryKind::Comment, 1_767_323_045));
    assert_eq!(e.url, "https://github.com/o/r/pull/7#c9");
}

#[test]
fn submit_review_creates_a_review() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/pulls/7/reviews" => Reply::json(r#"{"id":5}"#),
        _ => Reply::not_found(),
    });
    submit_review(&gh(), &token(Provider::GitHub), 7, ReviewEvent::Approve, "", None).unwrap();
    assert_eq!(find(&m, "POST").json(), serde_json::json!({ "event": "APPROVE", "body": "" }));
}

#[test]
fn submit_review_finishes_a_pending_review() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/pulls/7/reviews/99/events" => Reply::json(r#"{"id":99}"#),
        _ => Reply::not_found(),
    });
    submit_review(&gh(), &token(Provider::GitHub), 7, ReviewEvent::Comment, "Done", Some(99)).unwrap();
    assert_eq!(find(&m, "POST").json(), serde_json::json!({ "event": "COMMENT", "body": "Done" }));
}

fn merge_req(method: MergeMethod, title: &str, message: &str) -> MergeRequest {
    MergeRequest { method, head_sha: "h1".into(), title: title.into(), message: message.into() }
}

#[test]
fn merge_sends_method_sha_and_title() {
    let m = Mock::start(|r| match r.route().as_str() {
        "PUT api.github.com/repos/o/r/pulls/7/merge" => Reply::json(r#"{"sha":"m1","merged":true,"message":"Merged"}"#),
        _ => Reply::not_found(),
    });
    let (t, auth) = (gh(), token(Provider::GitHub));
    let done = merge(&t, &auth, 7, &merge_req(MergeMethod::Squash, "Add search (#7)", "")).unwrap();
    assert_eq!(done.sha, "m1");
    merge(&t, &auth, 7, &merge_req(MergeMethod::Rebase, "Ignored", "Ignored too")).unwrap();
    merge(&t, &auth, 7, &merge_req(MergeMethod::Merge, "", "Body")).unwrap();
    let bodies: Vec<serde_json::Value> = m.requests().iter().map(|r| r.json()).collect();
    assert_eq!(
        bodies,
        [
            serde_json::json!({ "merge_method": "squash", "sha": "h1", "commit_title": "Add search (#7)" }),
            serde_json::json!({ "merge_method": "rebase", "sha": "h1" }),
            serde_json::json!({ "merge_method": "merge", "sha": "h1", "commit_message": "Body" }),
        ]
    );
}

#[test]
fn merge_conflict_409_reads_as_branch_changed() {
    let e = error_for(409, r#"{"message":"Head branch was modified. Review and try the merge again."}"#, || {
        merge(&gh(), &token(Provider::GitHub), 7, &merge_req(MergeMethod::Merge, "", "")).map(drop)
    });
    assert!(matches!(&e, Error::Other(m) if m.starts_with("The source branch changed on the service")), "{e}");
}

#[test]
fn merge_405_carries_the_server_message() {
    let e = error_for(405, r#"{"message":"Pull Request is not mergeable"}"#, || {
        merge(&gh(), &token(Provider::GitHub), 7, &merge_req(MergeMethod::Merge, "", "")).map(drop)
    });
    assert!(matches!(&e, Error::Http { status: 405, message } if message.contains("Pull Request is not mergeable")), "{e}");
}

#[test]
fn merged_false_is_an_error() {
    let _m = Mock::start(|_| Reply::json(r#"{"sha":null,"merged":false,"message":"Base branch was modified"}"#));
    let e = merge(&gh(), &token(Provider::GitHub), 7, &merge_req(MergeMethod::Merge, "", "")).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "Base branch was modified"), "{e}");
}
