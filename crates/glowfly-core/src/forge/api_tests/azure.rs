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
            format!("GET {BASE}/_apis/git/repositories/web%20app/pullrequests?searchCriteria.status=active&$top=100&api-version=7.1");
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
        if route == format!("POST {BASE}/_apis/wit/wiql?$top=1000&api-version=7.1") {
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

const REPO: &str = "dev.azure.com/contoso/Fabrikam%20Web/_apis/git/repositories/web%20app";
const BUILD: &str = "0609b952-1397-4640-95ec-e00a01b2c241";

#[test]
fn pull_requests_carry_build_policies_and_statuses() {
    let m = Mock::start(|r| {
        let route = r.route();
        if route == format!("GET {REPO}/pullrequests?searchCriteria.status=active&$top=100&api-version=7.1") {
            Reply::json(PRS.replace(r#""lastMergeSourceCommit""#, r#""repository":{"project":{"id":"p-1"}},"lastMergeSourceCommit""#))
        } else if route
            == format!(
                "GET {BASE}/_apis/policy/evaluations?artifactId=vstfs%3A%2F%2F%2FCodeReview%2FCodeReviewId%2Fp-1%2F17&api-version=7.1-preview.1"
            )
        {
            Reply::json(format!(
                r#"{{"value":[
                  {{"status":"approved","configuration":{{"isEnabled":true,"type":{{"id":"{BUILD}"}}}},"context":{{"buildId":41}}}},
                  {{"status":"rejected","configuration":{{"isEnabled":true,"type":{{"id":"{BUILD}"}}}},"context":{{"buildId":42}}}},
                  {{"status":"rejected","configuration":{{"isEnabled":true,"type":{{"id":"fa4e907d-c16b-4a4c-9dfa-4906e5d171dd"}}}}}},
                  {{"status":"notApplicable","configuration":{{"isEnabled":true,"type":{{"id":"{BUILD}"}}}}}},
                  {{"status":"rejected","configuration":{{"isEnabled":false,"type":{{"id":"{BUILD}"}}}}}}]}}"#
            ))
        } else if route == format!("GET {REPO}/pullRequests/17/statuses?api-version=7.1") {
            Reply::json(
                r#"{"value":[{"id":1,"state":"pending","context":{"genre":"ci","name":"lint"},"targetUrl":"https://ci.example.com/1"},
                  {"id":2,"state":"succeeded","context":{"genre":"ci","name":"lint"},"targetUrl":"https://ci.example.com/2"}]}"#,
            )
        } else {
            Reply::not_found()
        }
    });
    let prs = pull_requests(&t(), &token(Provider::AzureDevOps)).unwrap();
    let c = prs[0].checks.as_ref().unwrap();
    assert_eq!((c.state, c.passed, c.failed, c.pending, c.complete), (CheckState::Failed, 2, 1, 0, true));
    assert_eq!(c.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=42");
    assert_eq!(routes(&m).len(), 3);
}

#[test]
fn commit_checks_use_branch_builds_and_statuses() {
    let sha = "c".repeat(40);
    let builds = format!(
        "GET {BASE}/_apis/build/builds?branchName=refs%2Fheads%2Ffeature%2F5-login&queryOrder=queueTimeDescending&$top=20&api-version=7.1"
    );
    let statuses = format!("GET {REPO}/commits/{sha}/statuses?latestOnly=true&api-version=7.1");
    let server = |deny_builds: bool| {
        let (builds, statuses, sha) = (builds.clone(), statuses.clone(), sha.clone());
        Mock::start(move |r| {
            let route = r.route();
            if route == builds {
                if deny_builds {
                    return Reply::status(403, r#"{"message":"no Build scope"}"#);
                }
                let b = |status: &str, result: &str, version: &str, repo: &str, def: u32, href: &str| {
                    format!(
                        r#"{{"status":"{status}","result":{result},"sourceVersion":"{version}","repository":{{"name":"{repo}"}},
                        "definition":{{"id":{def}}},"_links":{{"web":{{"href":"{href}"}}}}}}"#
                    )
                };
                let list = [
                    b("inProgress", "null", &sha, "web app", 1, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=9"),
                    b("completed", r#""failed""#, &sha, "web app", 1, "https://dev.azure.com/x/8"),
                    b("completed", r#""failed""#, &"d".repeat(40), "web app", 2, "https://dev.azure.com/x/7"),
                    b("completed", r#""failed""#, &sha, "other", 3, "https://dev.azure.com/x/6"),
                ];
                Reply::json(format!(r#"{{"value":[{}]}}"#, list.join(",")))
            } else if route == statuses {
                Reply::json(
                    r#"{"value":[{"id":5,"state":"succeeded","context":{"genre":"sec","name":"scan"},"targetUrl":"https://scan.example.com/5"}]}"#,
                )
            } else {
                Reply::not_found()
            }
        })
    };
    let tips = [TipQuery { sha: sha.clone(), branch: "feature/5-login".into() }];
    let m = server(false);
    let got = commit_checks(&t(), &token(Provider::AzureDevOps), &tips).unwrap();
    let c = &got[&sha];
    assert_eq!((c.state, c.passed, c.failed, c.pending), (CheckState::Pending, 1, 0, 1));
    assert_eq!(c.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=9");
    assert_eq!(routes(&m).len(), 2);
    drop(m);

    // Without access to builds the statuses still count.
    let _m = server(true);
    let got = commit_checks(&t(), &token(Provider::AzureDevOps), &tips).unwrap();
    let c = &got[&sha];
    assert_eq!((c.state, c.passed, c.url.as_str()), (CheckState::Passed, 1, "https://scan.example.com/5"));
}

#[test]
fn pr_details_read_the_project_once_then_each_pull_requests_policies() {
    let evaluations = |pr: u64| {
        format!(
            "GET {BASE}/_apis/policy/evaluations?artifactId=vstfs%3A%2F%2F%2FCodeReview%2FCodeReviewId%2Fp-1%2F{pr}&api-version=7.1-preview.1"
        )
    };
    let m = Mock::start(move |r| {
        let route = r.route();
        if route == format!("GET {REPO}?api-version=7.1") {
            Reply::json(r#"{"defaultBranch":"refs/heads/main","project":{"id":"p-1"}}"#)
        } else if route == evaluations(17) {
            Reply::json(format!(
                r#"{{"value":[{{"status":"running","configuration":{{"isEnabled":true,"type":{{"id":"{BUILD}"}}}},"context":{{"buildId":5}}}}]}}"#
            ))
        } else if route.contains("/statuses?") || route == evaluations(18) {
            Reply::json(r#"{"value":[]}"#)
        } else {
            Reply::not_found()
        }
    });
    let prs = [listed(17, Some("abc1234".into())), listed(18, None)];
    let got = pr_details(&t(), &token(Provider::AzureDevOps), &prs).unwrap();
    let c = got[0].checks.as_ref().unwrap();
    assert_eq!((c.state, c.url.as_str()), (CheckState::Pending, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=5"));
    assert_eq!(got[1].checks, None);
    let r = routes(&m);
    assert_eq!(r.iter().filter(|r| r.starts_with(&format!("GET {REPO}?"))).count(), 1);
    assert_eq!(r.iter().filter(|r| r.contains("/policy/evaluations?")).count(), 2);
    drop(m);

    // A repository answer without its project: no checks, and nothing more is asked.
    let m = Mock::start(|_| Reply::json(r#"{"defaultBranch":"refs/heads/main"}"#));
    let got = pr_details(&t(), &token(Provider::AzureDevOps), &prs).unwrap();
    assert!(got.len() == 2 && got.iter().all(|d| d.checks.is_none()));
    assert_eq!(routes(&m).len(), 1);
}
