use super::*;

const PRS: &str = r#"[
  {"number":4,"title":"Add search","body":"Closes #9","draft":true,"html_url":"https://github.com/o/r/pull/4",
   "user":{"login":"ada"},"head":{"ref":"feature/search","sha":"abcdef1234567"},"base":{"ref":"main","sha":"x"},
   "requested_reviewers":[{"login":"linus"}]},
  {"number":5,"title":"Sneaky","body":null,"html_url":"https://github.com.evil.example/o/r/pull/5",
   "user":{"login":"eve"},"head":{"ref":"x","sha":""},"base":{"ref":"main","sha":"x"}}
]"#;

const REVIEWS: &str = r#"{"data":{"repository":{
  "p4":{"reviews":{"nodes":[{"author":{"login":"grace"},"state":"CHANGES_REQUESTED"},{"author":{"login":"grace"},"state":"APPROVED"},
    {"author":{"login":"bob"},"state":"COMMENTED"},{"author":null,"state":"APPROVED"}]},
    "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"FAILURE","contexts":{"totalCount":3,"nodes":[
      {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://ci.example.com/ok","url":"https://github.com/o/r/runs/1"},
      {"__typename":"CheckRun","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":"https://ci.example.com/build/7","url":"https://github.com/o/r/runs/2"},
      {"__typename":"StatusContext","state":"SUCCESS","targetUrl":"https://ci.example.com/s"}]}}}}]}},
  "p5":{"reviews":{"nodes":[]},"commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]}}}}}"#;

/// `graphql` is the GraphQL endpoint's route.
fn pr_server(api_host: &'static str, prefix: &'static str, graphql: &'static str) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        if route == graphql {
            return Reply::json(REVIEWS);
        }
        match route.strip_prefix(&format!("GET {api_host}{prefix}/repos/o/r")) {
            Some("/pulls?state=open&per_page=100") => Reply::json(PRS),
            _ => Reply::not_found(),
        }
    })
}

/// A list of `count` pull requests, each with one requested reviewer.
fn many_prs(count: usize) -> String {
    let pr = |n: usize| {
        format!(
            r#"{{"number":{n},"title":"P","html_url":"https://github.com/o/r/pull/{n}","user":{{"login":"a"}},
            "head":{{"ref":"b","sha":""}},"base":{{"ref":"main","sha":""}},"requested_reviewers":[{{"login":"linus"}}]}}"#
        )
    };
    format!("[{}]", (1..=count).map(pr).collect::<Vec<_>>().join(","))
}

#[test]
fn lists_pull_requests_with_review_states() {
    let m = pr_server("api.github.com", "", "POST api.github.com/graphql");
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.len(), 2);
    let p = &prs[0];
    assert_eq!((p.id, p.title.as_str(), p.source.as_str(), p.target.as_str(), p.draft), (4, "Add search", "feature/search", "main", true));
    assert_eq!((p.author.as_str(), p.description.as_str(), p.url.as_str()), ("ada", "Closes #9", "https://github.com/o/r/pull/4"));
    let states: Vec<(&str, &str)> = p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
    assert_eq!(states, [("grace", "approved"), ("linus", "requested")]);
    assert!(p.head_matches("abcdef1234567"));
    let c = p.checks.as_ref().unwrap();
    assert_eq!((c.state, c.passed, c.failed, c.pending, c.complete), (CheckState::Failed, 2, 1, 0, true));
    // The failing run's page is the one offered.
    assert_eq!(c.url, "https://ci.example.com/build/7");
    // A link to another host is dropped; the rest of the pull request stays.
    assert_eq!((prs[1].id, prs[1].url.as_str(), prs[1].head.as_deref()), (5, "", None));
    // No CI on this one.
    assert_eq!(prs[1].checks, None);

    let list = find(&m, "GET api.github.com/repos/o/r/pulls?");
    assert_eq!(list.header("Authorization"), Some("Bearer tok"));
    assert_eq!(list.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    assert_eq!(list.header("Accept"), Some("application/json"));
    // Every pull request's reviews and checks came in one GraphQL call, with owner and repository passed
    // as variables rather than written into the query.
    assert_eq!(routes(&m).len(), 2);
    let sent = find(&m, "POST api.github.com/graphql").json();
    let query = sent["query"].as_str().unwrap();
    assert!(query.contains("p4: pullRequest(number: 4)") && query.contains("p5: pullRequest(number: 5)"), "{query}");
    assert!(query.contains("statusCheckRollup") && query.contains("commits(last: 1)"), "{query}");
    assert_eq!(sent["variables"], serde_json::json!({"owner":"o","repo":"r"}));
}

#[test]
fn reviews_are_batched_and_a_failed_lookup_leaves_votes_empty() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(many_prs(120)),
        "POST api.github.com/graphql" => Reply::json(r#"{"data":{"repository":{}}}"#),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.len(), 120);
    // 120 pull requests: three calls of up to 50.
    assert_eq!(routes(&m).iter().filter(|r| r.contains("graphql")).count(), 3);
    drop(m);

    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(many_prs(120)),
        "POST api.github.com/graphql" => Reply::status(502, "bad gateway"),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert!(prs.iter().all(|p| p.reviewers.len() == 1 && p.reviewers[0].state == "requested"));
    assert!(prs.iter().all(|p| p.checks.is_none()));
    // The first batch failed, so the rest weren't asked for.
    assert_eq!(routes(&m).iter().filter(|r| r.contains("graphql")).count(), 1);
}

/// GraphQL calls `m` received, and how many of them asked for checks.
fn graphql_calls(m: &Mock) -> (usize, Vec<bool>) {
    let bodies: Vec<bool> = m.requests().iter().filter(|r| r.method == "POST").map(|r| r.body.contains("statusCheckRollup")).collect();
    (bodies.len(), bodies)
}

#[test]
fn checks_are_dropped_when_the_server_rejects_the_rollup_field() {
    let server = |list: String| {
        Mock::start(move |r| match r.route().as_str() {
            "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(list.clone()),
            "POST api.github.com/graphql" if r.body.contains("statusCheckRollup") => {
                Reply::json(r#"{"errors":[{"message":"no such field"}]}"#)
            }
            "POST api.github.com/graphql" => Reply::json(REVIEWS),
            _ => Reply::not_found(),
        })
    };
    let m = server(PRS.into());
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    let states: Vec<(&str, &str)> = prs[0].reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
    assert_eq!(states, [("grace", "approved"), ("linus", "requested")]);
    assert!(prs.iter().all(|p| p.checks.is_none()));
    assert_eq!(graphql_calls(&m), (2, vec![true, false]));
    drop(m);

    // The first batch is asked twice; the rest skip the check fields straight away.
    let m = server(many_prs(120));
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.len(), 120);
    assert_eq!(graphql_calls(&m), (4, vec![true, false, false, false]));
}

#[test]
fn a_field_error_on_the_rollup_keeps_reviews() {
    // A token that may read pull requests but not statuses: only that field is nulled.
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(PRS),
        "POST api.github.com/graphql" => Reply::json(
            r#"{"data":{"repository":{"p4":{"reviews":{"nodes":[{"author":{"login":"grace"},"state":"APPROVED"}]},
              "commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]}}}},
            "errors":[{"type":"FORBIDDEN","path":["repository","p4","commits","nodes",0,"commit","statusCheckRollup"],
              "message":"Resource not accessible by personal access token"}]}"#,
        ),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs[0].approvals(), 1);
    assert!(prs.iter().all(|p| p.checks.is_none()));
    assert_eq!(graphql_calls(&m), (1, vec![true]));
}

#[test]
fn enterprise_server_uses_its_own_host_and_api_path() {
    let m = pr_server("github.acme.io", "/api/v3", "POST github.acme.io/api/graphql");
    let t = remote("https://github.acme.io/o/r.git");
    assert_eq!(t.provider, Provider::GitHubEnterprise);
    let prs = pull_requests(&t, &token(Provider::GitHubEnterprise)).unwrap();
    // github.com links don't belong to this server.
    assert_eq!((prs.len(), prs[0].url.as_str()), (2, ""));
    assert_eq!(routes(&m), ["GET github.acme.io/api/v3/repos/o/r/pulls?state=open&per_page=100", "POST github.acme.io/api/graphql"]);
    assert_eq!(prs[0].approvals(), 1);
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
        source_repo: None,
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

fn sha(n: u8) -> String {
    format!("{n:x}").repeat(40)
}

fn tip(sha: String, branch: &str) -> TipQuery {
    TipQuery { sha, branch: branch.into() }
}

#[test]
fn commit_checks_are_batched_by_oid() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/graphql" => Reply::json(
            r#"{"data":{"repository":{
              "c0":{"statusCheckRollup":{"state":"PENDING","contexts":{"totalCount":1,"nodes":[
                {"__typename":"CheckRun","status":"IN_PROGRESS","conclusion":null,"detailsUrl":"javascript:alert(1)"}]}}},
              "c1":null}}}"#,
        ),
        _ => Reply::not_found(),
    });
    let tips = [tip(sha(1), "main"), tip("abc1234".into(), "short"), tip(sha(2), "gone")];
    let got = commit_checks(&remote("https://github.com/o/r"), &token(Provider::GitHub), &tips).unwrap();
    assert_eq!(got.len(), 1);
    let c = &got[&sha(1)];
    // An unsafe link is dropped.
    assert_eq!((c.state, c.pending, c.url.as_str()), (CheckState::Pending, 1, ""));
    let sent = find(&m, "POST api.github.com/graphql").json();
    let query = sent["query"].as_str().unwrap();
    assert!(query.contains("c0: object(oid: $o0)") && query.contains("... on Commit") && query.contains("$o1: GitObjectID!"), "{query}");
    // The short id was never sent.
    assert_eq!(sent["variables"], serde_json::json!({"owner":"o","repo":"r","o0":sha(1),"o1":sha(2)}));
    assert!(!query.contains("$o2"), "{query}");
    // Nothing to ask: no call at all.
    let none = commit_checks(&remote("https://github.com/o/r"), &token(Provider::GitHub), &[tip("HEAD".into(), "x")]).unwrap();
    assert!(none.is_empty());
    assert_eq!(routes(&m).len(), 1);
}

#[test]
fn commit_checks_unsupported_server() {
    let _m =
        Mock::start(|_| Reply::json(r#"{"data":{"repository":null},"errors":[{"message":"Field 'statusCheckRollup' doesn't exist"}]}"#));
    let t = remote("https://github.acme.io/o/r");
    let err = commit_checks(&t, &token(Provider::GitHubEnterprise), &[tip(sha(3), "main")]).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)), "{err}");
}

#[test]
fn later_batches_apply_reviews() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(many_prs(120)),
        "POST api.github.com/graphql" => {
            // Every alias in the query gets one approval.
            let query = r.json()["query"].as_str().unwrap_or_default().to_owned();
            let approved = r#"{"reviews":{"nodes":[{"author":{"login":"grace"},"state":"APPROVED"}]}}"#;
            let aliases: Vec<String> = query
                .split("pullRequest(number: ")
                .skip(1)
                .filter_map(|rest| rest.split(')').next())
                .map(|n| format!(r#""p{n}":{approved}"#))
                .collect();
            Reply::json(format!(r#"{{"data":{{"repository":{{{}}}}}}}"#, aliases.join(",")))
        }
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.len(), 120);
    assert!(prs.iter().all(|p| p.approvals() == 1));
    // The third call's batch (101 to 120) included.
    assert_eq!(prs[100..].iter().map(|p| p.id).collect::<Vec<_>>(), (101..=120).collect::<Vec<_>>());
    assert_eq!(routes(&m).iter().filter(|r| r.contains("graphql")).count(), 3);
}
