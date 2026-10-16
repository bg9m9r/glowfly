use super::*;

const MRS: &str = r#"[
  {"iid":3,"title":"Draft: speed up","description":"Closes #2","source_branch":"2-speed","target_branch":"main",
   "sha":"0123456789abcdef","author":{"name":"Ada","username":"ada"},"draft":true,"web_url":"https://gitlab.acme.io/group/sub/app/-/merge_requests/3",
   "reviewers":[{"name":"Grace","username":"g"},{"name":"Linus","username":"l"}],"detailed_merge_status":"mergeable"},
  {"iid":4,"title":"Elsewhere","source_branch":"x","target_branch":"main","author":{"name":"Eve"},
   "web_url":"https://gitlab.com/group/sub/app/-/merge_requests/4"}
]"#;

#[test]
fn lists_merge_requests_with_approvals_on_a_self_managed_server() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.acme.io/api/v4/projects/group%2Fsub%2Fapp/merge_requests?state=opened&per_page=100" => Reply::json(MRS),
        "GET gitlab.acme.io/api/v4/projects/group%2Fsub%2Fapp/merge_requests/3/approvals" => {
            Reply::json(r#"{"approved_by":[{"user":{"name":"Grace","username":"g"}}]}"#)
        }
        _ => Reply::status(500, r#"{"message":"boom"}"#),
    });
    let t = remote("https://gitlab.acme.io/group/sub/app.git");
    assert_eq!(t.provider, Provider::GitLabSelfManaged);
    let prs = pull_requests(&t, &token(Provider::GitLabSelfManaged)).unwrap();
    let p = &prs[0];
    assert_eq!((p.id, p.draft, p.author.as_str(), p.merge_status.as_deref()), (3, true, "Ada", Some("mergeable")));
    let states: Vec<(&str, &str)> = p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
    assert_eq!(states, [("Grace", "approved"), ("Linus", "no vote")]);
    assert_eq!(p.url, "https://gitlab.acme.io/group/sub/app/-/merge_requests/3");
    assert!(prs.iter().all(|p| p.checks.is_none()));
    // An approvals lookup that fails leaves that merge request without reviewers; a link to
    // another host is dropped.
    assert_eq!((prs[1].reviewers.len(), prs[1].url.as_str()), (0, ""));

    let list = find(&m, "GET gitlab.acme.io/api/v4/projects/group%2Fsub%2Fapp/merge_requests?");
    assert_eq!(list.header("Authorization"), Some("Bearer tok"));
    assert_eq!(list.header("X-GitHub-Api-Version"), None);
}

#[test]
fn creates_a_draft_merge_request() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST gitlab.com/api/v4/projects/g%2Fapp/merge_requests" => Reply::status(
            201,
            r#"{"iid":8,"title":"Draft: New","source_branch":"topic","target_branch":"main","author":{"name":"Ada"},
            "draft":true,"web_url":"https://gitlab.com/g/app/-/merge_requests/8"}"#,
        ),
        _ => Reply::not_found(),
    });
    let new = NewPullRequest {
        source: "topic".into(),
        target: "main".into(),
        title: "New".into(),
        description: "Body".into(),
        draft: true,
        issues: Vec::new(),
    };
    let pr = create_pull_request(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab), &new).unwrap();
    assert_eq!((pr.id, pr.draft, pr.url.as_str()), (8, true, "https://gitlab.com/g/app/-/merge_requests/8"));
    let sent = find(&m, "POST gitlab.com/api/v4/projects/g%2Fapp/merge_requests").json();
    assert_eq!(sent["title"], "Draft: New");
    assert_eq!((sent["source_branch"].as_str(), sent["target_branch"].as_str()), (Some("topic"), Some("main")));
    assert_eq!(sent["remove_source_branch"], false);
}

#[test]
fn assigned_and_looked_up_issues() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects/g%2Fapp/issues?state=opened&scope=assigned_to_me&per_page=100&order_by=updated_at" => Reply::json(
            r#"[{"iid":2,"title":"Slow","state":"opened","web_url":"https://gitlab.com/g/app/-/issues/2","labels":["perf"],
                "assignee":{"name":"Ada"}}]"#,
        ),
        "GET gitlab.com/api/v4/projects/g%2Fapp/issues?scope=all&per_page=100&iids[]=2&iids[]=5" => Reply::json(
            r#"[{"iid":5,"title":"Old","state":"closed","web_url":"https://gitlab.com/g/app/-/issues/5","issue_type":"incident"}]"#,
        ),
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://gitlab.com/g/app"), token(Provider::GitLab));
    let mine = my_issues(&t, &auth).unwrap();
    assert_eq!((mine[0].key.as_str(), mine[0].kind.as_str(), mine[0].assignee.as_deref()), ("2", "perf", Some("Ada")));
    let keys = ["2", "5", "x"].map(String::from);
    let found = issues(&t, &auth, &keys).unwrap();
    assert_eq!((found[0].key.as_str(), found[0].kind.as_str(), found[0].state.as_str()), ("5", "incident", "closed"));
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn forbidden_is_reported_as_such() {
    let _m = Mock::start(|_| Reply::status(403, r#"{"message":"403 Forbidden"}"#));
    let err = default_branch(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab)).unwrap_err();
    assert!(matches!(err, Error::Forbidden(_)), "{err}");
}
