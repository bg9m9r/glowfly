use super::*;
use crate::forge::filters::{self, Conditions, FilterKind, Me, Milestone, Who};

fn created_by_me() -> Conditions {
    Builtin::IssuesCreatedByMe.conditions()
}

#[test]
fn github_searches_issues_as_the_signed_in_user() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada","name":"Ada"}"#),
        "GET api.github.com/repos/o/r/issues?state=open&per_page=100&sort=updated&creator=ada" => Reply::json(
            r#"[{"number":9,"title":"Crash","state":"open","html_url":"https://github.com/o/r/issues/9","labels":[{"name":"bug"}]},
            {"number":4,"title":"PR","state":"open","html_url":"https://github.com/o/r/pull/4","pull_request":{"url":"x"}}]"#,
        ),
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://github.com/o/r"), token(Provider::GitHub));
    let found = filters::find_issues(&t, &auth, &created_by_me(), None).unwrap();
    assert_eq!(found.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["9"], "pull requests are dropped");
    assert_eq!(found[0].facets.labels, ["bug"]);
    assert_eq!(routes(&m).len(), 2);
    drop(m);

    // Knowing the user already saves the lookup.
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/issues?state=open&per_page=100&sort=updated&creator=ada" => Reply::json("[]"),
        _ => Reply::not_found(),
    });
    let me = Me { id: "ada".into(), name: "Ada".into() };
    assert!(filters::find_issues(&t, &auth, &created_by_me(), Some(&me)).unwrap().is_empty());
    assert_eq!(routes(&m), ["GET api.github.com/repos/o/r/issues?state=open&per_page=100&sort=updated&creator=ada"]);
}

#[test]
fn gitlab_and_bitbucket_searches() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/user" => Reply::json(r#"{"name":"Ada","username":"ada"}"#),
        route if route.starts_with("GET gitlab.com/api/v4/projects/g%2Fapp/issues?") => Reply::json(
            r#"[{"iid":2,"title":"Slow","state":"opened","web_url":"https://gitlab.evil.example/x","labels":[],"milestone":{"title":"m"}}]"#,
        ),
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"display_name":"Ada","uuid":"{u-1}"}"#),
        route if route.starts_with("GET api.bitbucket.org/2.0/repositories/team/app/issues?q=") => Reply::json(r#"{"values":[]}"#),
        _ => Reply::not_found(),
    });
    let found = filters::find_issues(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab), &created_by_me(), None).unwrap();
    assert_eq!((found[0].key.as_str(), found[0].facets.milestone.as_deref()), ("2", Some("m")));
    assert_eq!(found[0].url, "", "a link to another host is blanked");
    let gl = find(&m, "GET gitlab.com/api/v4/projects/g%2Fapp/issues?").path;
    assert!(gl.contains("&scope=all&author_username=ada"), "{gl}");

    let bb = filters::find_issues(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket), &created_by_me(), None);
    assert!(bb.unwrap().is_empty());
    let q = find(&m, "GET api.bitbucket.org/2.0/repositories/team/app/issues?q=").path;
    assert!(q.contains("reporter.uuid%3D%22%7Bu-1%7D%22"), "{q}");
}

#[test]
fn azure_searches_with_wiql() {
    let base = "dev.azure.com/contoso/Fabrikam%20Web";
    let m = Mock::start(move |r| {
        let route = r.route();
        if route == format!("POST {base}/_apis/wit/wiql?$top=1000&api-version=7.1") {
            Reply::json(r#"{"workItems":[{"id":5}]}"#)
        } else if route.starts_with(&format!("GET {base}/_apis/wit/workitems?ids=5&")) {
            Reply::json(r#"{"value":[{"id":5,"fields":{"System.Title":"Login","System.Tags":"ux"}}]}"#)
        } else {
            Reply::not_found()
        }
    });
    let t = remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app");
    let found = filters::find_issues(&t, &token(Provider::AzureDevOps), &created_by_me(), None).unwrap();
    assert_eq!((found[0].key.as_str(), found[0].facets.labels.as_slice()), ("5", &["ux".to_owned()][..]));
    let query = find(&m, "POST dev.azure.com/").json()["query"].as_str().unwrap().to_owned();
    assert!(query.contains("[System.CreatedBy] = @Me") && !query.contains("AssignedTo"), "{query}");
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn jira_searches_with_jql_on_cloud_and_data_center() {
    let m = Mock::start(|_| Reply::json(r#"{"issues":[{"key":"P-1","fields":{"summary":"S","labels":["ui"]}}]}"#));
    let cloud = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let c = Conditions { labels: vec!["ui".into()], ..created_by_me() };
    let found = filters::find_issues(&Target::jira(&cloud, "P"), &token(Provider::JiraCloud), &c, None).unwrap();
    assert_eq!(found[0].facets.labels, ["ui"]);
    let path = find(&m, "GET acme.atlassian.net/rest/api/3/search/jql?").path;
    assert!(path.contains(&enc(r#"statusCategory != Done AND project = "P" AND reporter = currentUser() AND labels = "ui""#)), "{path}");
    drop(m);

    let m = Mock::start(|_| Reply::json(r#"{"startAt":0,"total":0,"issues":[]}"#));
    let dc = Account { provider: Provider::JiraDataCenter, host: "jira.acme.io".into(), org: String::new() };
    filters::find_issues(&Target::jira(&dc, ""), &token(Provider::JiraDataCenter), &created_by_me(), None).unwrap();
    assert!(routes(&m)[0].starts_with("GET jira.acme.io/rest/api/2/search?jql="), "{:?}", routes(&m));
}

#[test]
fn unsupported_conditions_send_nothing() {
    let m = Mock::start(|_| Reply::json("[]"));
    let c = Builtin::IssuesMentioningMe.conditions();
    let e = filters::find_issues(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab), &c, None).unwrap_err();
    assert!(matches!(&e, Error::Unsupported(why) if why.contains("GitLab")), "{e}");
    let labels = Conditions { labels: vec!["x".into()], ..Default::default() };
    assert!(filters::find_issues(&remote("https://bitbucket.org/t/a"), &token(Provider::Bitbucket), &labels, None).is_err());
    assert!(routes(&m).is_empty());
}

#[test]
fn picker_options_per_service() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/labels?per_page=100" => Reply::json(r#"[{"name":"bug"},{"name":"ui"}]"#),
        "GET api.github.com/repos/o/r/milestones?state=open&per_page=100" => Reply::json(r#"[{"number":3,"title":"v1"}]"#),
        "GET gitlab.com/api/v4/projects/g%2Fapp/labels?per_page=100" => Reply::json(r#"[{"name":"perf"}]"#),
        "GET gitlab.com/api/v4/projects/g%2Fapp/milestones?state=active&per_page=100" => Reply::json(r#"[{"title":"16.0"}]"#),
        "GET acme.atlassian.net/rest/api/3/label?maxResults=1000" => Reply::json(r#"{"values":["ui"]}"#),
        "GET acme.atlassian.net/rest/api/3/project/P/versions" => Reply::json(r#"[{"name":"1.0","released":true},{"name":"2.0"}]"#),
        "GET acme.atlassian.net/rest/agile/1.0/board?projectKeyOrId=P&type=scrum" => Reply::json(r#"{"values":[{"id":1},{"id":2}]}"#),
        "GET acme.atlassian.net/rest/agile/1.0/board/1/sprint?state=active,future" => Reply::json(r#"{"values":[{"id":42,"name":"S9"}]}"#),
        "GET acme.atlassian.net/rest/agile/1.0/board/2/sprint?state=active,future" => {
            Reply::json(r#"{"values":[{"id":42,"name":"S9"},{"id":43,"name":"S10"}]}"#)
        }
        _ => Reply::not_found(),
    });
    let gh = filters::filter_options(&remote("https://github.com/o/r"), &token(Provider::GitHub), FilterKind::Prs).unwrap();
    assert_eq!(gh.labels, ["bug", "ui"]);
    assert_eq!((gh.milestones[0].key.as_str(), gh.milestones[0].title.as_str()), ("3", "v1"));
    let gl = filters::filter_options(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab), FilterKind::Issues).unwrap();
    assert_eq!((gl.labels.as_slice(), gl.milestones[0].key.as_str()), (&["perf".to_owned()][..], "16.0"));
    let cloud = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let jira = filters::filter_options(&Target::jira(&cloud, "P"), &token(Provider::JiraCloud), FilterKind::Issues).unwrap();
    assert_eq!(jira.labels, ["ui"]);
    assert_eq!(jira.milestones.iter().map(|v| v.title.as_str()).collect::<Vec<_>>(), ["2.0"], "unreleased only");
    assert_eq!(jira.sprints.iter().map(|s| (s.key.as_str(), s.title.as_str())).collect::<Vec<_>>(), [("42", "S9"), ("43", "S10")]);
    assert!(routes(&m).iter().all(|r| !r.contains("evil")));
}

#[test]
fn the_signed_in_user_on_each_code_host() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada","name":""}"#),
        "GET gitlab.com/api/v4/user" => Reply::json(r#"{"name":"Ada L","username":"ada"}"#),
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"display_name":"Ada","uuid":"{u-1}"}"#),
        "GET bb.acme.io/plugins/servlet/applinks/whoami" => Reply::text("ada\n"),
        "GET dev.azure.com/contoso/_apis/connectionData?api-version=7.1-preview" => {
            Reply::json(r#"{"authenticatedUser":{"id":"a-1","providerDisplayName":"Ada"}}"#)
        }
        _ => Reply::not_found(),
    });
    let who = |url: &str, p: Provider| {
        let m = filters::me(&remote(url), &token(p)).unwrap();
        (m.id, m.name)
    };
    assert_eq!(who("https://github.com/o/r", Provider::GitHub), ("ada".into(), "ada".into()));
    assert_eq!(who("https://gitlab.com/g/app", Provider::GitLab), ("ada".into(), "Ada L".into()));
    assert_eq!(who("https://bitbucket.org/t/a", Provider::Bitbucket), ("{u-1}".into(), "Ada".into()));
    assert_eq!(who("https://dev.azure.com/contoso/p/_git/r", Provider::AzureDevOps), ("a-1".into(), "Ada".into()));
    let dc = Target {
        provider: Provider::BitbucketDataCenter,
        host: "bb.acme.io".into(),
        owner: "P".into(),
        project: String::new(),
        repo: "r".into(),
    };
    assert_eq!(filters::me(&dc, &token(Provider::BitbucketDataCenter)).unwrap().id, "ada");
    let unassigned = Conditions { assignee: Who::Nobody, milestone: Milestone::None, ..Default::default() };
    assert!(filters::matching::needs_me(FilterKind::Issues, &created_by_me()));
    assert!(!filters::matching::needs_me(FilterKind::Issues, &unassigned));
}
