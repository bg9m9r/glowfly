//! Issue details, Create issue options and creating issues on GitHub, GitHub Enterprise Server
//! and GitLab, and Bitbucket Data Center, which has no issue tracker of its own (Jira,
//! Bitbucket Cloud, Azure DevOps and Trello have their own files).

use super::*;

pub(super) fn new_issue(title: &str) -> NewIssue {
    NewIssue {
        project: String::new(),
        title: title.into(),
        description: String::new(),
        labels: Vec::new(),
        assignees: Vec::new(),
        kind: None,
        priority: None,
        epic: None,
    }
}

const GH_ISSUE: &str = r#"{"number":9,"title":"Crash on start","state":"open","html_url":"https://github.com/o/r/issues/9",
    "labels":[{"name":"bug"},{"name":"ui"}],"assignee":{"login":"ada"},"milestone":{"title":"v1"},
    "body":"It crashes.","user":{"login":"grace"},"created_at":"2026-01-02T10:00:00Z",
    "assignees":[{"login":"ada"},{"login":"linus"}]}"#;

const GH_COMMENTS: &str = r#"[{"user":{"login":"ada"},"body":"Seen it","created_at":"2026-01-03T10:00:00Z","html_url":"https://github.com/o/r/issues/9#issuecomment-1"},
    {"user":null,"body":"me too","created_at":"2026-01-04T10:00:00Z","html_url":"https://evil.example/c/2"}]"#;

#[test]
fn github_issue_details_with_comments() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/issues/9" => Reply::json(GH_ISSUE),
        "GET api.github.com/repos/o/r/issues/9/comments?per_page=100" => Reply::json(GH_COMMENTS),
        _ => Reply::not_found(),
    });
    let d = issue_details(&remote("https://github.com/o/r"), &token(Provider::GitHub), "9").unwrap();
    assert_eq!(routes(&m), ["GET api.github.com/repos/o/r/issues/9", "GET api.github.com/repos/o/r/issues/9/comments?per_page=100"]);
    assert!(m.requests().iter().all(|r| r.header("Authorization") == Some("Bearer tok")));
    assert_eq!((d.issue.key.as_str(), d.issue.title.as_str(), d.issue.state.as_str()), ("9", "Crash on start", "open"));
    assert_eq!(d.issue.facets.labels, ["bug", "ui"]);
    assert_eq!((d.body.as_str(), d.author.as_str(), d.created), ("It crashes.", "grace", 1_767_348_000));
    assert_eq!(d.assignees, ["ada", "linus"]);
    assert_eq!(d.comments.len(), 2);
    assert_eq!((d.comments[0].author.as_str(), d.comments[0].body.as_str()), ("ada", "Seen it"));
    assert_eq!(d.comments[0].url, "https://github.com/o/r/issues/9#issuecomment-1");
    assert_eq!((d.comments[1].author.as_str(), d.comments[1].url.as_str()), ("ghost", ""), "a deleted user; a link elsewhere is blanked");
    assert!(d.comments.iter().all(|c| c.kind == EntryKind::Comment));
}

#[test]
fn github_pull_requests_and_odd_keys_are_refused() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/issues/7" => {
            Reply::json(r#"{"number":7,"title":"PR","state":"open","html_url":"https://github.com/o/r/pull/7","pull_request":{"url":"x"}}"#)
        }
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://github.com/o/r"), token(Provider::GitHub));
    let e = issue_details(&t, &auth, "7").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("pull request"), "{e}");
    assert_eq!(routes(&m), ["GET api.github.com/repos/o/r/issues/7"], "no comments are read for it");
    let e = issue_details(&t, &auth, "8").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Issue #8 wasn't found"), "{e}");
    let before = routes(&m).len();
    assert!(issue_details(&t, &auth, "../x").unwrap_err().is_not_found());
    assert_eq!(routes(&m).len(), before, "a key that isn't a number sends nothing");
}

#[test]
fn github_enterprise_uses_its_api_path() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET git.acme.io/api/v3/repos/o/r/issues/9" => Reply::json(GH_ISSUE),
        "GET git.acme.io/api/v3/repos/o/r/issues/9/comments?per_page=100" => Reply::json("[]"),
        _ => Reply::not_found(),
    });
    let t = Target {
        provider: Provider::GitHubEnterprise,
        host: "git.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    let d = issue_details(&t, &token(Provider::GitHubEnterprise), "9").unwrap();
    assert_eq!(d.issue.url, "", "a github.com link doesn't belong to this server");
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn github_create_options_and_create() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/labels?per_page=100" => Reply::json(r#"[{"name":"bug"},{"name":"ui"}]"#),
        "GET api.github.com/repos/o/r/assignees?per_page=100" => Reply::json(r#"[{"login":"ada"},{"login":"linus"}]"#),
        "POST api.github.com/repos/o/r/issues" => Reply::json(
            r#"{"number":12,"title":"New one","state":"open","html_url":"https://github.com/o/r/issues/12","labels":[{"name":"bug"}]}"#,
        ),
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://github.com/o/r"), token(Provider::GitHub));
    let o = issue_create_options(&t, &auth, "").unwrap();
    assert_eq!(o.labels, ["bug", "ui"]);
    assert_eq!(o.people, [Person { id: "ada".into(), name: "ada".into() }, Person { id: "linus".into(), name: "linus".into() }]);
    assert_eq!((o.max_assignees, o.labels_free, o.types.len(), o.epics.len()), (10, false, 0, 0));

    let new = NewIssue { labels: vec!["bug".into()], assignees: vec!["ada".into()], description: "Steps".into(), ..new_issue("New one") };
    let i = create_issue(&t, &auth, &new).unwrap();
    assert_eq!((i.key.as_str(), i.url.as_str()), ("12", "https://github.com/o/r/issues/12"));
    let body = find(&m, "POST api.github.com/repos/o/r/issues").json();
    assert_eq!(body, serde_json::json!({ "title": "New one", "body": "Steps", "labels": ["bug"], "assignees": ["ada"] }));
    drop(m);

    let m = Mock::start(|_| Reply::json(r#"{"number":13,"title":"Bare","state":"open","html_url":"https://evil.example/13"}"#));
    let i = create_issue(&t, &auth, &new_issue("Bare")).unwrap();
    assert_eq!(i.url, "", "the link is checked");
    assert_eq!(m.requests()[0].json(), serde_json::json!({ "title": "Bare" }), "empty lists and body are left out");
    drop(m);

    let m = Mock::start(|_| Reply::json("{}"));
    let e = create_issue(&t, &auth, &new_issue("  ")).unwrap_err();
    assert!(e.to_string().contains("Enter a title."), "{e}");
    assert!(m.requests().is_empty());
}

#[test]
fn gitlab_details_options_and_create() {
    let base = "gitlab.com/api/v4/projects/g%2Fapp";
    let m = Mock::start(move |r| {
        let route = r.route();
        let path = route.strip_prefix(&format!("{} {base}", r.method)).unwrap_or_default().to_owned();
        match (r.method.as_str(), path.as_str()) {
            ("GET", "/issues/2") => Reply::json(
                r#"{"iid":2,"title":"Slow","state":"opened","web_url":"https://gitlab.com/g/app/-/issues/2","labels":["perf"],
                "description":"Takes ages","author":{"name":"Ada","username":"ada"},"created_at":"2026-01-02T10:00:00.000Z",
                "assignees":[{"name":"","username":"grace"}]}"#,
            ),
            ("GET", "/issues/2/notes?sort=asc&order_by=created_at&per_page=100") => Reply::json(
                r#"[{"id":5,"body":"added label perf","author":{"name":"Ada","username":"ada"},"created_at":"2026-01-02T11:00:00Z","system":true},
                {"id":6,"body":"On it","author":{"name":"Grace","username":"grace"},"created_at":"2026-01-03T10:00:00Z","system":false}]"#,
            ),
            ("GET", "/labels?per_page=100") => Reply::json(r#"[{"name":"perf"},{"name":"ui"}]"#),
            ("GET", "/members/all?per_page=100") => Reply::json(
                r#"[{"id":5,"name":"Ada","username":"ada"},{"id":7,"name":"","username":"bob"},{"id":5,"name":"Ada","username":"ada"}]"#,
            ),
            ("POST", "/issues") => {
                Reply::json(r#"{"iid":3,"title":"Outage","state":"opened","web_url":"https://gitlab.com/g/app/-/issues/3"}"#)
            }
            _ => Reply::not_found(),
        }
    });
    let (t, auth) = (remote("https://gitlab.com/g/app"), token(Provider::GitLab));
    let d = issue_details(&t, &auth, "2").unwrap();
    assert_eq!((d.issue.key.as_str(), d.body.as_str(), d.author.as_str()), ("2", "Takes ages", "Ada"));
    assert_eq!(d.assignees, ["grace"]);
    assert_eq!(d.comments.len(), 1, "system notes are left out");
    assert_eq!((d.comments[0].author.as_str(), d.comments[0].body.as_str()), ("Grace", "On it"));
    assert_eq!(d.comments[0].url, "https://gitlab.com/g/app/-/issues/2#note_6");

    let o = issue_create_options(&t, &auth, "").unwrap();
    assert_eq!(o.labels, ["perf", "ui"]);
    assert_eq!(o.people, [Person { id: "5".into(), name: "Ada".into() }, Person { id: "7".into(), name: "bob".into() }]);
    assert_eq!(
        o.types.iter().map(|n| (n.key.as_str(), n.title.as_str())).collect::<Vec<_>>(),
        [("issue", "Issue"), ("incident", "Incident")]
    );
    assert_eq!((o.max_assignees, o.labels_free), (1, true));

    let new = NewIssue {
        labels: vec!["a".into(), "b".into()],
        assignees: vec!["5".into(), "not-a-number".into()],
        kind: Some("incident".into()),
        ..new_issue("Outage")
    };
    let i = create_issue(&t, &auth, &new).unwrap();
    assert_eq!(i.key, "3");
    let body = find(&m, &format!("POST {base}/issues")).json();
    assert_eq!(body, serde_json::json!({ "title": "Outage", "labels": "a,b", "assignee_ids": [5], "issue_type": "incident" }));
}

#[test]
fn bitbucket_data_center_sends_nothing() {
    let m = Mock::start(|_| Reply::json("{}"));
    let p = Provider::BitbucketDataCenter;
    let t = Target { provider: p, host: "example.com".into(), owner: "o".into(), project: "p".into(), repo: "r".into() };
    let auth = token(p);
    let errors = [
        issue_details(&t, &auth, "1").unwrap_err(),
        issue_create_options(&t, &auth, "p").unwrap_err(),
        create_issue(&t, &auth, &new_issue("Title")).unwrap_err(),
    ];
    for e in errors {
        let expected = "Bitbucket Data Center has no issue tracker of its own; use Jira for its issues";
        assert!(matches!(&e, Error::Unsupported(msg) if msg == expected), "{e}");
    }
    assert!(m.requests().is_empty());
}
