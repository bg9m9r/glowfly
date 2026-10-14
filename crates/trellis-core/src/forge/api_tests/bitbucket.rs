use super::*;
use base64::Engine;

fn basic(user: &str, pass: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")))
}

const CLOUD_PRS: &str = r#"{"values":[
  {"id":12,"title":"Fix","description":"","source":{"branch":{"name":"7-fix"},"commit":{"hash":"0123456789ab"}},
   "destination":{"branch":{"name":"main"}},"author":{"display_name":"Ada"},"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/12"}},
   "participants":[{"user":{"display_name":"Grace"},"role":"REVIEWER","approved":true},
                   {"user":{"display_name":"Bob"},"role":"PARTICIPANT","state":"changes_requested"}]},
  {"id":13,"title":"Off-site","source":{"branch":{"name":"y"}},"destination":{"branch":{"name":"main"}},
   "links":{"html":{"href":"http://bitbucket.org/team/app/pull-requests/13"}}}
]}"#;

#[test]
fn cloud_pull_requests_with_app_password() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/team/app/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants" => {
            Reply::json(CLOUD_PRS)
        }
        _ => Reply::not_found(),
    });
    let auth = Auth::from_token(Provider::Bitbucket, Some("ada"), "app-pass", "test");
    let prs = pull_requests(&remote("git@bitbucket.org:team/app.git"), &auth).unwrap();
    let p = &prs[0];
    assert_eq!((p.id, p.source.as_str(), p.target.as_str(), p.author.as_str()), (12, "7-fix", "main", "Ada"));
    let states: Vec<(&str, &str)> = p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
    assert_eq!(states, [("Grace", "approved"), ("Bob", "changes requested")]);
    assert!(p.head_matches("0123456789abcdef"));
    // Plain-http links are dropped.
    assert_eq!((prs[1].id, prs[1].url.as_str()), (13, ""));
    let req = find(&m, "GET api.bitbucket.org/");
    assert_eq!(req.header("Authorization"), Some(basic("ada", "app-pass").as_str()));
    assert_eq!(req.header("X-GitHub-Api-Version"), None);
}

#[test]
fn cloud_issues_and_a_disabled_tracker() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"display_name":"Ada","uuid":"{u-1}"}"#),
        route if route.starts_with("GET api.bitbucket.org/2.0/repositories/team/app/issues?q=") => Reply::json(
            r#"{"values":[{"id":3,"title":"Crash","state":"open","kind":"bug","assignee":{"display_name":"Ada"},
            "links":{"html":{"href":"https://bitbucket.org/team/app/issues/3"}}}]}"#,
        ),
        "GET api.bitbucket.org/2.0/repositories/team/app/issues/3" => Reply::json(
            r#"{"id":3,"title":"Crash","state":"resolved","kind":"bug","links":{"html":{"href":"https://bitbucket.org/team/app/issues/3"}}}"#,
        ),
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://bitbucket.org/team/app"), token(Provider::Bitbucket));
    let mine = my_issues(&t, &auth).unwrap();
    assert_eq!(
        (mine[0].key.as_str(), mine[0].kind.as_str(), mine[0].url.as_str()),
        ("3", "bug", "https://bitbucket.org/team/app/issues/3")
    );
    let q = find(&m, "GET api.bitbucket.org/2.0/repositories/team/app/issues?q=").path;
    assert!(q.contains("assignee.uuid%3D%22%7Bu-1%7D%22"), "{q}");
    let found = issues(&t, &auth, &["3".into(), "4".into()]).unwrap();
    assert_eq!(found.iter().map(|i| i.state.as_str()).collect::<Vec<_>>(), ["resolved"]);

    // A repository with its issue tracker turned off answers 404: no issues, no error.
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"display_name":"Ada","uuid":"{u-1}"}"#),
        _ => Reply::not_found(),
    });
    assert!(my_issues(&t, &auth).unwrap().is_empty());
}

#[test]
fn cloud_creates_a_pull_request() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.bitbucket.org/2.0/repositories/team/app/pullrequests" => Reply::status(
            201,
            r#"{"id":20,"title":"New","source":{"branch":{"name":"topic"}},"destination":{"branch":{"name":"main"}},
            "links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/20"}}}"#,
        ),
        _ => Reply::not_found(),
    });
    let new = NewPullRequest {
        source: "topic".into(),
        target: "main".into(),
        title: "New".into(),
        description: "Body".into(),
        draft: false,
        issues: Vec::new(),
    };
    let pr = create_pull_request(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket), &new).unwrap();
    assert_eq!((pr.id, pr.url.as_str()), (20, "https://bitbucket.org/team/app/pull-requests/20"));
    let sent = find(&m, "POST api.bitbucket.org/").json();
    assert_eq!(sent["source"]["branch"]["name"], "topic");
    assert_eq!(sent["destination"]["branch"]["name"], "main");
}

const DC_PRS: &str = r#"{"isLastPage":true,"values":[
  {"id":3,"title":"Port","fromRef":{"displayId":"PROJ-1-port","latestCommit":"deadbeef00"},"toRef":{"displayId":"master"},
   "author":{"user":{"displayName":"Ada"},"status":"UNAPPROVED"},"reviewers":[{"user":{"displayName":"Linus"},"status":"NEEDS_WORK"}],
   "links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/3"}]}}
]}"#;

#[test]
fn data_center_pull_requests_verify_and_create() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests?state=OPEN&limit=100" => Reply::json(DC_PRS),
        "GET bitbucket.acme.io/plugins/servlet/applinks/whoami" => Reply::text("ada\n"),
        "POST bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests" => Reply::status(201, {
            let v: serde_json::Value = serde_json::from_str(DC_PRS).unwrap();
            v["values"][0].to_string()
        }),
        _ => Reply::not_found(),
    });
    let t = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    let auth = token(Provider::BitbucketDataCenter);
    let prs = pull_requests(&t, &auth).unwrap();
    let p = &prs[0];
    assert_eq!((p.source.as_str(), p.target.as_str(), p.reviewers[0].state.as_str()), ("PROJ-1-port", "master", "changes requested"));
    assert_eq!(p.url, "https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/3");
    assert_eq!(verify(&t.account(), &auth).unwrap(), "ada");
    let new = NewPullRequest {
        source: "PROJ-1-port".into(),
        target: "master".into(),
        title: "Port".into(),
        description: String::new(),
        draft: false,
        issues: Vec::new(),
    };
    create_pull_request(&t, &auth, &new).unwrap();
    let sent = find(&m, "POST bitbucket.acme.io/").json();
    assert_eq!(sent["fromRef"]["id"], "refs/heads/PROJ-1-port");
    assert_eq!(sent["toRef"]["repository"], serde_json::json!({"slug":"api","project":{"key":"PROJ"}}));
    // Data Center has no issue tracker: nothing is asked.
    let before = routes(&m).len();
    assert!(my_issues(&t, &auth).unwrap().is_empty());
    assert_eq!(routes(&m).len(), before);
}

#[test]
fn data_center_sign_in_page_is_not_a_user_name() {
    let _m = Mock::start(|_| Reply::text("<!DOCTYPE html><html>log in</html>"));
    let t = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    assert!(verify(&t.account(), &token(Provider::BitbucketDataCenter)).unwrap_err().is_auth());
}
