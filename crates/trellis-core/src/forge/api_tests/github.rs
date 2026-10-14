use super::*;

const PRS: &str = r#"[
  {"number":4,"title":"Add search","body":"Closes #9","draft":true,"html_url":"https://github.com/o/r/pull/4",
   "user":{"login":"ada"},"head":{"ref":"feature/search","sha":"abcdef1234567"},"base":{"ref":"main","sha":"x"},
   "requested_reviewers":[{"login":"linus"}]},
  {"number":5,"title":"Sneaky","body":null,"html_url":"https://github.com.evil.example/o/r/pull/5",
   "user":{"login":"eve"},"head":{"ref":"x","sha":""},"base":{"ref":"main","sha":"x"}}
]"#;

const REVIEWS_4: &str = r#"[{"user":{"login":"grace"},"state":"CHANGES_REQUESTED"},{"user":{"login":"grace"},"state":"APPROVED"},
  {"user":{"login":"bob"},"state":"COMMENTED"},{"user":null,"state":"APPROVED"}]"#;

fn pr_server(api_host: &'static str, prefix: &'static str) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        let Some(path) = route.strip_prefix(&format!("GET {api_host}{prefix}/repos/o/r")) else { return Reply::not_found() };
        match path {
            "/pulls?state=open&per_page=100" => Reply::json(PRS),
            "/pulls/4/reviews?per_page=100" => Reply::json(REVIEWS_4),
            "/pulls/5/reviews?per_page=100" => Reply::json("[]"),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn lists_pull_requests_with_review_states() {
    let m = pr_server("api.github.com", "");
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.len(), 2);
    let p = &prs[0];
    assert_eq!((p.id, p.title.as_str(), p.source.as_str(), p.target.as_str(), p.draft), (4, "Add search", "feature/search", "main", true));
    assert_eq!((p.author.as_str(), p.description.as_str(), p.url.as_str()), ("ada", "Closes #9", "https://github.com/o/r/pull/4"));
    let states: Vec<(&str, &str)> = p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
    assert_eq!(states, [("grace", "approved"), ("linus", "requested")]);
    assert!(p.head_matches("abcdef1234567"));
    // A link to another host is dropped; the rest of the pull request stays.
    assert_eq!((prs[1].id, prs[1].url.as_str(), prs[1].head.as_deref()), (5, "", None));

    let list = find(&m, "GET api.github.com/repos/o/r/pulls?");
    assert_eq!(list.header("Authorization"), Some("Bearer tok"));
    assert_eq!(list.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    assert_eq!(list.header("Accept"), Some("application/json"));
}

#[test]
fn enterprise_server_uses_its_own_host_and_api_path() {
    let m = pr_server("github.acme.io", "/api/v3");
    let t = remote("https://github.acme.io/o/r.git");
    assert_eq!(t.provider, Provider::GitHubEnterprise);
    let prs = pull_requests(&t, &token(Provider::GitHubEnterprise)).unwrap();
    // github.com links don't belong to this server.
    assert_eq!((prs.len(), prs[0].url.as_str()), (2, ""));
    assert!(routes(&m).iter().all(|r| r.starts_with("GET github.acme.io/api/v3/repos/o/r/pulls")), "{:?}", routes(&m));
}

#[test]
fn assigned_issues_skip_pull_requests() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada lovelace","name":"Ada"}"#),
        "GET api.github.com/repos/o/r/issues?state=open&assignee=ada%20lovelace&per_page=100&sort=updated" => Reply::json(
            r#"[{"number":9,"title":"Crash","state":"open","html_url":"https://github.com/o/r/issues/9","labels":[{"name":"bug"}],"assignee":{"login":"ada"}},
            {"number":4,"title":"PR","state":"open","html_url":"https://github.com/o/r/pull/4","pull_request":{"url":"x"}},
            {"number":7,"title":"Elsewhere","state":"open","html_url":"javascript:alert(1)"}]"#,
        ),
        _ => Reply::not_found(),
    });
    let issues = my_issues(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    let got: Vec<(&str, &str, &str, &str)> =
        issues.iter().map(|i| (i.key.as_str(), i.title.as_str(), i.kind.as_str(), i.url.as_str())).collect();
    assert_eq!(got, [("9", "Crash", "bug", "https://github.com/o/r/issues/9"), ("7", "Elsewhere", "Issue", "")]);
    assert_eq!(issues[0].assignee.as_deref(), Some("ada"));
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn issues_by_key_skip_unknown_and_non_numeric_keys() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/issues/9" => {
            Reply::json(r#"{"number":9,"title":"Crash","state":"closed","html_url":"https://github.com/o/r/issues/9"}"#)
        }
        _ => Reply::not_found(),
    });
    let keys = ["9", "10", "PROJ-1"].map(String::from);
    let found = issues(&remote("https://github.com/o/r"), &token(Provider::GitHub), &keys).unwrap();
    assert_eq!(found.iter().map(|i| (i.key.as_str(), i.state.as_str())).collect::<Vec<_>>(), [("9", "closed")]);
    assert_eq!(routes(&m), ["GET api.github.com/repos/o/r/issues/9", "GET api.github.com/repos/o/r/issues/10"]);
}

#[test]
fn creates_a_pull_request() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/pulls" => Reply::status(
            201,
            r#"{"number":12,"title":"New","draft":true,"html_url":"https://github.com/o/r/pull/12",
            "user":{"login":"ada"},"head":{"ref":"topic","sha":"0123456789"},"base":{"ref":"main","sha":"y"}}"#,
        ),
        _ => Reply::not_found(),
    });
    let new = NewPullRequest {
        source: "topic".into(),
        target: "main".into(),
        title: "New".into(),
        description: "Body".into(),
        draft: true,
        issues: vec!["3".into()],
    };
    let pr = create_pull_request(&remote("https://github.com/o/r"), &token(Provider::GitHub), &new).unwrap();
    assert_eq!((pr.id, pr.url.as_str(), pr.draft), (12, "https://github.com/o/r/pull/12", true));
    let sent = find(&m, "POST api.github.com/repos/o/r/pulls").json();
    assert_eq!(sent, serde_json::json!({"title":"New","head":"topic","base":"main","body":"Body","draft":true}));
}

#[test]
fn verifies_and_reads_the_default_branch() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada","name":""}"#),
        "GET api.github.com/repos/o/r" => Reply::json(r#"{"default_branch":"trunk"}"#),
        _ => Reply::not_found(),
    });
    let t = remote("https://github.com/o/r");
    let auth = token(Provider::GitHub);
    assert_eq!(verify(&t.account(), &auth).unwrap(), "ada");
    assert_eq!(default_branch(&t, &auth).unwrap().as_deref(), Some("trunk"));
}
