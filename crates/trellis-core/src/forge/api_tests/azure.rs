use super::*;
use base64::Engine;

const BASE: &str = "dev.azure.com/contoso/Fabrikam%20Web";

fn t() -> Target {
    remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app")
}

const PRS: &str = r#"{"count":1,"value":[{"pullRequestId":17,"title":"Add login","description":"AB#5",
  "sourceRefName":"refs/heads/feature/5-login","targetRefName":"refs/heads/main","isDraft":true,"createdBy":{"displayName":"Ada"},
  "reviewers":[{"displayName":"Grace","vote":10,"isRequired":true},{"displayName":"Bob","vote":-5}],"mergeStatus":"succeeded",
  "lastMergeSourceCommit":{"commitId":"abc1234"}}]}"#;

#[test]
fn pull_requests_with_votes_and_built_links() {
    let m = Mock::start(|r| {
        let list =
            format!("GET {BASE}/_apis/git/repositories/web%20app/pullrequests?searchCriteria.status=active&$top=200&api-version=7.1");
        if r.route() == list { Reply::json(PRS) } else { Reply::not_found() }
    });
    let prs = pull_requests(&t(), &token(Provider::AzureDevOps)).unwrap();
    let p = &prs[0];
    assert_eq!((p.id, p.source.as_str(), p.target.as_str(), p.draft), (17, "feature/5-login", "main", true));
    let states: Vec<(&str, &str, bool)> = p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str(), r.required)).collect();
    assert_eq!(states, [("Grace", "approved", true), ("Bob", "waiting for author", false)]);
    assert_eq!(p.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app/pullrequest/17");
    // A personal access token is sent as Basic auth with an empty user name.
    let expected = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(":tok"));
    let req = find(&m, "GET dev.azure.com/");
    assert_eq!(req.header("Authorization"), Some(expected.as_str()));
    assert_eq!(req.header("X-GitHub-Api-Version"), None);
}

#[test]
fn creates_a_pull_request_linking_work_items() {
    let m = Mock::start(|r| {
        let create = format!("POST {BASE}/_apis/git/repositories/web%20app/pullrequests?api-version=7.1");
        if r.route() == create {
            let v: serde_json::Value = serde_json::from_str(PRS).unwrap();
            Reply::status(201, v["value"][0].to_string())
        } else {
            Reply::not_found()
        }
    });
    let new = NewPullRequest {
        source: "feature/5-login".into(),
        target: "main".into(),
        title: "Add login".into(),
        description: "AB#5".into(),
        draft: true,
        issues: vec!["5".into(), "PROJ-1".into()],
    };
    let pr = create_pull_request(&t(), &token(Provider::AzureDevOps), &new).unwrap();
    assert_eq!(pr.id, 17);
    let sent = find(&m, "POST dev.azure.com/").json();
    assert_eq!(sent["sourceRefName"], "refs/heads/feature/5-login");
    assert_eq!(sent["isDraft"], true);
    assert_eq!(sent["workItemRefs"], serde_json::json!([{"id":"5"}]));
}

#[test]
fn assigned_work_items_keep_the_query_order() {
    let m = Mock::start(|r| {
        let route = r.route();
        if route == format!("POST {BASE}/_apis/wit/wiql?$top=100&api-version=7.1") {
            Reply::json(r#"{"workItems":[{"id":8},{"id":5}]}"#)
        } else if route.starts_with(&format!("GET {BASE}/_apis/wit/workitems?ids=8,5&")) {
            Reply::json(
                r#"{"value":[{"id":5,"fields":{"System.Title":"Login","System.State":"Active","System.WorkItemType":"User Story",
                "System.AssignedTo":{"displayName":"Ada"}}},null,{"id":8,"fields":{"System.Title":"Crash","System.State":"New","System.WorkItemType":"Bug"}}]}"#,
            )
        } else {
            Reply::not_found()
        }
    });
    let items = my_issues(&t(), &token(Provider::AzureDevOps)).unwrap();
    let got: Vec<(&str, &str, &str)> = items.iter().map(|i| (i.key.as_str(), i.kind.as_str(), i.url.as_str())).collect();
    assert_eq!(
        got,
        [
            ("8", "Bug", "https://dev.azure.com/contoso/Fabrikam%20Web/_workitems/edit/8"),
            ("5", "User Story", "https://dev.azure.com/contoso/Fabrikam%20Web/_workitems/edit/5")
        ]
    );
    let wiql = find(&m, "POST dev.azure.com/").json();
    assert!(wiql["query"].as_str().unwrap().contains("@Me"));
}

#[test]
fn a_sign_in_page_is_an_auth_error_and_anonymous_is_refused() {
    let sign_in = Mock::start(|_| Reply::status(203, "<html>sign in</html>"));
    assert!(pull_requests(&t(), &token(Provider::AzureDevOps)).unwrap_err().is_auth());
    drop(sign_in);
    let _m = Mock::start(|r| {
        if r.route().starts_with("GET dev.azure.com/contoso/_apis/connectionData?") {
            Reply::json(r#"{"authenticatedUser":{"providerDisplayName":"Anonymous","descriptor":"System:PublicAccess;x"}}"#)
        } else {
            Reply::not_found()
        }
    });
    assert!(verify(&t().account(), &token(Provider::AzureDevOps)).unwrap_err().is_auth());
}
