//! Each check on a pull request's head commit, for the pull request panel: GitHub's status
//! rollup (with the fallback for servers without `isRequired`), GitLab's head pipeline jobs,
//! Azure DevOps build policies and statuses, and Bitbucket build statuses.

use super::*;
use serde_json::{Value, json};

fn q(id: u64, head: Option<&str>) -> CheckQuery {
    CheckQuery { id, head: head.map(str::to_owned) }
}

fn names(list: &CheckList) -> Vec<&str> {
    list.runs.iter().map(|r| r.name.as_str()).collect()
}

fn find_run<'a>(list: &'a CheckList, name: &str) -> &'a CheckRun {
    list.runs.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("{name}: {:?}", names(list)))
}

/// A GraphQL answer with one page of contexts.
fn rollup_page(nodes: Value, next: Option<&str>) -> String {
    json!({ "data": { "repository": { "pullRequest": { "commits": { "nodes": [{ "commit": { "oid": "abc123",
        "statusCheckRollup": { "state": "FAILURE", "contexts": { "totalCount": 4,
            "pageInfo": { "hasNextPage": next.is_some(), "endCursor": next }, "nodes": nodes } } } }] } } } } })
    .to_string()
}

fn first_page() -> Value {
    json!([
        { "__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS",
          "startedAt": "2026-01-02T10:00:00Z", "completedAt": "2026-01-02T10:02:14Z",
          "detailsUrl": null, "url": "https://github.com/o/r/runs/1", "title": "Built", "isRequired": true,
          "checkSuite": { "workflowRun": { "workflow": { "name": "CI" } }, "app": { "name": "GitHub Actions" } } },
        { "__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE",
          "detailsUrl": "javascript:alert(1)", "url": "https://github.com/o/r/runs/2", "isRequired": false,
          "checkSuite": { "workflowRun": null, "app": { "name": "Linter" } } }
    ])
}

fn second_page() -> Value {
    json!([
        { "__typename": "StatusContext", "context": "ci/deploy", "state": "SUCCESS", "description": "Deployed",
          "targetUrl": "http://ci.example/deploy", "createdAt": "2026-01-02T11:00:00Z", "isRequired": false },
        { "__typename": "StatusContext", "context": "ci/e2e", "state": "PENDING", "targetUrl": "https://ci.example/e2e",
          "createdAt": "2026-01-02T11:00:00Z", "isRequired": true },
        { "__typename": "Deployment" }
    ])
}

#[test]
fn github_lists_runs_statuses_and_required_contexts() {
    let m = Mock::start(|r| {
        if r.route() != "POST api.github.com/graphql" {
            return Reply::not_found();
        }
        let vars = &r.json()["variables"];
        match vars["after"].as_str() {
            None => Reply::json(rollup_page(first_page(), Some("c1"))),
            Some("c1") => Reply::json(rollup_page(second_page(), None)),
            Some(_) => Reply::not_found(),
        }
    });
    let list = check_list(&remote("https://github.com/o/r"), &token(Provider::GitHub), &q(7, Some("abc123"))).unwrap();
    let sent = m.requests();
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|r| r.json()["variables"]["number"] == 7 && r.json()["variables"]["owner"] == "o"));
    assert!(sent[0].json()["variables"]["after"].is_null() && sent[1].json()["variables"]["after"] == "c1");
    assert!(sent[0].json()["query"].as_str().unwrap().contains("isRequired(pullRequestNumber: $number)"));
    assert_eq!((list.sha.as_str(), list.complete), ("abc123", true));
    // Failed first, then the running ones (required first), then the passed ones.
    assert_eq!(names(&list), ["lint", "ci/e2e", "build", "ci/deploy"]);
    let build = find_run(&list, "build");
    assert_eq!((build.group.as_deref(), build.required, build.state), (Some("CI"), Some(true), RunState::Passed));
    assert_eq!((build.started, build.finished), (Some(1_767_348_000), Some(1_767_348_134)));
    assert_eq!((build.url.as_str(), build.description.as_str()), ("https://github.com/o/r/runs/1", "Built"));
    let lint = find_run(&list, "lint");
    assert_eq!((lint.group.as_deref(), lint.required, lint.url.as_str()), (Some("Linter"), Some(false), ""));
    let deploy = find_run(&list, "ci/deploy");
    assert_eq!((deploy.group.as_deref(), deploy.url.as_str(), deploy.finished), (None, "", Some(1_767_351_600)));
    let e2e = find_run(&list, "ci/e2e");
    assert_eq!((e2e.state, e2e.finished, e2e.required, e2e.url.as_str()), (RunState::Pending, None, Some(true), "https://ci.example/e2e"));
}

#[test]
fn github_enterprise_without_is_required_retries_without_it() {
    let m = Mock::start(|r| {
        if r.route() != "POST ghe.acme.io/api/graphql" {
            return Reply::not_found();
        }
        if r.body.contains("isRequired") {
            return Reply::json(r#"{"errors":[{"message":"Field 'isRequired' doesn't exist on type 'CheckRun'"}]}"#);
        }
        let mut nodes = first_page();
        for n in nodes.as_array_mut().unwrap() {
            n.as_object_mut().unwrap().remove("isRequired");
        }
        Reply::json(rollup_page(nodes, None))
    });
    let t = Target {
        provider: Provider::GitHubEnterprise,
        host: "ghe.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    let list = check_list(&t, &token(Provider::GitHubEnterprise), &q(7, None)).unwrap();
    let sent = m.requests();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].body.contains("isRequired") && !sent[1].body.contains("isRequired"));
    assert_eq!(names(&list), ["lint", "build"]);
    assert!(list.runs.iter().all(|r| r.required.is_none()));

    // No rollup at all (nothing reported yet): an empty list.
    let _m = Mock::start(|_| {
        Reply::json(r#"{"data":{"repository":{"pullRequest":{"commits":{"nodes":[{"commit":{"oid":"h1","statusCheckRollup":null}}]}}}}}"#)
    });
    let list = check_list(&t, &token(Provider::GitHubEnterprise), &q(7, None)).unwrap();
    assert_eq!((list.sha.as_str(), list.runs.len(), list.summary().as_str()), ("h1", 0, "No build status"));
}

const MR_ROUTE: &str = "GET gitlab.com/api/v4/projects/g%2Fapp/merge_requests/7";
const JOBS_ROUTE: &str = "GET gitlab.com/api/v4/projects/g%2Fapp/pipelines/55/jobs?per_page=100";

#[test]
fn gitlab_lists_head_pipeline_jobs() {
    let m = Mock::start(|r| match r.route().as_str() {
        MR_ROUTE => Reply::json(
            r#"{"sha":"abc","head_pipeline":{"id":55,"sha":"abc","status":"failed","web_url":"https://gitlab.com/g/app/-/pipelines/55"}}"#,
        ),
        JOBS_ROUTE => Reply::json(
            r#"[{"id":104,"name":"deploy","stage":"deploy","status":"manual","allow_failure":false,"web_url":"https://gitlab.com/g/app/-/jobs/104"},
            {"id":103,"name":"lint","stage":"test","status":"failed","allow_failure":true,"failure_reason":"script_failure",
             "started_at":"2026-01-02T10:00:00.000Z","finished_at":"2026-01-02T10:00:45.000Z","web_url":"https://gitlab.com/g/app/-/jobs/103"},
            {"id":102,"name":"unit","stage":"test","status":"success","allow_failure":false},
            {"id":101,"name":"compile","stage":"build","status":"success","allow_failure":false,
             "started_at":"2026-01-02T09:58:00Z","finished_at":"2026-01-02T09:59:00Z"}]"#,
        ),
        _ => Reply::not_found(),
    });
    let t = remote("https://gitlab.com/g/app");
    let list = check_list(&t, &token(Provider::GitLab), &q(7, None)).unwrap();
    assert_eq!(routes(&m), [MR_ROUTE, JOBS_ROUTE]);
    assert_eq!(list.sha, "abc");
    // Failed, then manual, then the passed jobs in pipeline order.
    assert_eq!(names(&list), ["lint", "deploy", "compile", "unit"]);
    let lint = find_run(&list, "lint");
    assert_eq!((lint.state, lint.required, lint.group.as_deref()), (RunState::Failed, Some(false), Some("test")));
    assert_eq!(lint.description, "script failure (allowed to fail)");
    assert_eq!((lint.started, lint.finished), (Some(1_767_348_000), Some(1_767_348_045)));
    let deploy = find_run(&list, "deploy");
    assert_eq!((deploy.state, deploy.required), (RunState::ActionRequired, Some(true)));

    // No head pipeline: nothing to list, and no jobs asked for.
    let m = Mock::start(|r| match r.route().as_str() {
        MR_ROUTE => Reply::json(r#"{"sha":"def","head_pipeline":null}"#),
        _ => Reply::not_found(),
    });
    let list = check_list(&t, &token(Provider::GitLab), &q(7, None)).unwrap();
    assert_eq!((list.sha.as_str(), list.runs.len()), ("def", 0));
    assert_eq!(routes(&m), [MR_ROUTE]);
}

const AZ_BASE: &str = "dev.azure.com/contoso/Fabrikam%20Web";
const AZ_REPO: &str = "dev.azure.com/contoso/Fabrikam%20Web/_apis/git/repositories/web%20app";
const BUILD_POLICY_ID: &str = "0609b952-1397-4640-95ec-e00a01b2c241";

fn azure_server(build: Reply) -> Mock {
    let build = std::sync::Mutex::new(Some(build));
    Mock::start(move |r| {
        let route = r.route();
        let evaluations = format!(
            "GET {AZ_BASE}/_apis/policy/evaluations?artifactId=vstfs%3A%2F%2F%2FCodeReview%2FCodeReviewId%2Fp-1%2F17&api-version=7.1-preview.1"
        );
        if route == format!("GET {AZ_REPO}?api-version=7.1") {
            Reply::json(r#"{"project":{"id":"p-1"}}"#)
        } else if route == evaluations {
            Reply::json(format!(
                r#"{{"value":[
                {{"status":"rejected","configuration":{{"isEnabled":true,"isBlocking":true,"type":{{"id":"{BUILD_POLICY_ID}"}},"settings":{{"displayName":""}}}},"context":{{"buildId":42}}}},
                {{"status":"notApplicable","configuration":{{"isEnabled":true,"isBlocking":false,"type":{{"id":"{BUILD_POLICY_ID}"}},"settings":{{"displayName":"Docs build"}}}}}},
                {{"status":"approved","configuration":{{"isEnabled":true,"isBlocking":true,"type":{{"id":"cbdc66da-9728-4af8-aada-9a5a32e4a226"}},"settings":{{"statusName":"lint","statusGenre":"ci"}}}}}},
                {{"status":"approved","configuration":{{"isEnabled":false,"isBlocking":true,"type":{{"id":"{BUILD_POLICY_ID}"}},"settings":{{"displayName":"Off"}}}}}}]}}"#
            ))
        } else if route == format!("GET {AZ_BASE}/_apis/build/builds/42?api-version=7.1") {
            build.lock().unwrap().take().unwrap_or_else(Reply::not_found)
        } else if route == format!("GET {AZ_REPO}/pullRequests/17/statuses?api-version=7.1") {
            Reply::json(
                r#"{"value":[
                {"id":1,"state":"pending","context":{"genre":"ci","name":"lint"}},
                {"id":3,"state":"failed","context":{"genre":"ci","name":"lint"},"description":"2 problems","targetUrl":"https://ci.example/lint",
                 "updatedDate":"2026-01-02T10:00:00Z"},
                {"id":2,"state":"succeeded","context":{"genre":"","name":"coverage"}}]}"#,
            )
        } else {
            Reply::not_found()
        }
    })
}

#[test]
fn azure_lists_build_policies_and_statuses() {
    let t = remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app");
    let build = Reply::json(
        r#"{"result":"canceled","status":"completed","startTime":"2026-01-02T09:00:00Z","finishTime":"2026-01-02T09:03:00Z",
        "definition":{"name":"web-ci"},"_links":{"web":{"href":"https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=42&view=logs"}}}"#,
    );
    let _m = azure_server(build);
    let list = check_list(&t, &token(Provider::AzureDevOps), &q(17, None)).unwrap();
    assert_eq!(names(&list), ["ci/lint", "web-ci", "coverage", "Docs build"]);
    let ci = find_run(&list, "web-ci");
    assert_eq!(
        (ci.state, ci.required, ci.started, ci.finished),
        (RunState::Cancelled, Some(true), Some(1_767_344_400), Some(1_767_344_580))
    );
    assert_eq!(ci.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=42&view=logs");
    let docs = find_run(&list, "Docs build");
    assert_eq!((docs.state, docs.required, docs.url.as_str()), (RunState::Skipped, Some(false), ""));
    // The latest status of a context is kept, and a blocking status policy makes it required.
    let lint = find_run(&list, "ci/lint");
    assert_eq!((lint.state, lint.required, lint.description.as_str()), (RunState::Failed, Some(true), "2 problems"));
    assert_eq!(lint.finished, Some(1_767_348_000));
    assert_eq!(find_run(&list, "coverage").required, Some(false));

    // Without the Build scope: the evaluation alone, linked to the build's results page.
    let _m = azure_server(Reply::status(403, r#"{"message":"no access"}"#));
    let list = check_list(&t, &token(Provider::AzureDevOps), &q(17, None)).unwrap();
    let ci = find_run(&list, "Build");
    assert_eq!((ci.state, ci.required), (RunState::Failed, Some(true)));
    assert_eq!(ci.url, "https://dev.azure.com/contoso/Fabrikam%20Web/_build/results?buildId=42");
}

#[test]
fn bitbucket_cloud_lists_pull_request_statuses() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/team/app/pullrequests/12/statuses?pagelen=100" => Reply::json(
            r#"{"values":[{"state":"SUCCESSFUL","key":"build","name":"Build","url":"https://ci.example/1",
            "created_on":"2026-01-02T10:00:00Z","updated_on":"2026-01-02T10:01:00Z"}],
            "next":"https://api.bitbucket.org/2.0/repositories/team/app/pullrequests/12/statuses?pagelen=100&page=2"}"#,
        ),
        "GET api.bitbucket.org/2.0/repositories/team/app/pullrequests/12/statuses?pagelen=100&page=2" => Reply::json(
            r#"{"values":[{"state":"STOPPED","key":"deploy-7","url":"https://ci.example/2","description":"Stopped by ada",
            "created_on":"2026-01-02T10:00:00Z","updated_on":"2026-01-02T10:05:00Z"},
            {"state":"INPROGRESS","key":"e2e","name":"E2E","created_on":"2026-01-02T10:00:00Z","updated_on":"2026-01-02T10:02:00Z"}]}"#,
        ),
        _ => Reply::not_found(),
    });
    let list = check_list(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket), &q(12, None)).unwrap();
    assert_eq!(routes(&m).len(), 2);
    assert_eq!(names(&list), ["E2E", "deploy-7", "Build"]);
    let deploy = find_run(&list, "deploy-7");
    assert_eq!((deploy.state, deploy.description.as_str()), (RunState::Cancelled, "Stopped by ada"));
    assert_eq!((deploy.started, deploy.finished), (Some(1_767_348_000), Some(1_767_348_300)));
    let e2e = find_run(&list, "E2E");
    assert_eq!((e2e.state, e2e.started, e2e.finished), (RunState::Running, Some(1_767_348_000), None));
    assert!(list.runs.iter().all(|r| r.required.is_none()));
}

#[test]
fn bitbucket_dc_lists_commit_build_statuses() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let first = format!("GET bitbucket.acme.io/rest/build-status/1.0/commits/{sha}?limit=100");
    let second = format!("{first}&start=1");
    let m = Mock::start({
        let (first, second) = (first.clone(), second.clone());
        move |r| {
            let route = r.route();
            if route == first {
                Reply::json(
                    r#"{"isLastPage":false,"nextPageStart":1,"values":[{"state":"INPROGRESS","key":"ci","name":"CI","url":"https://ci.acme.io/1","dateAdded":1767348000000}]}"#,
                )
            } else if route == second {
                Reply::json(
                    r#"{"isLastPage":true,"values":[{"state":"FAILED","key":"lint","url":"https://ci.acme.io/2","dateAdded":1767348060000}]}"#,
                )
            } else {
                Reply::not_found()
            }
        }
    });
    let t = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    let auth = token(Provider::BitbucketDataCenter);
    let list = check_list(&t, &auth, &q(3, Some(sha))).unwrap();
    assert_eq!(routes(&m), [first, second]);
    assert_eq!((list.sha.as_str(), names(&list)), (sha, vec!["lint", "CI"]));
    let ci = find_run(&list, "CI");
    assert_eq!((ci.state, ci.started, ci.finished), (RunState::Running, Some(1_767_348_000), None));
    let lint = find_run(&list, "lint");
    assert_eq!((lint.state, lint.started, lint.finished), (RunState::Failed, None, Some(1_767_348_060)));

    // A head that isn't a full commit id: nothing is asked.
    let before = routes(&m).len();
    let list = check_list(&t, &auth, &q(3, Some("0123abc"))).unwrap();
    assert!(list.runs.is_empty() && list.complete);
    assert_eq!(routes(&m).len(), before);
}

#[test]
fn other_services_have_no_check_list() {
    let m = Mock::start(|_| Reply::json("{}"));
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let e = check_list(&Target::jira(&account, ""), &token(Provider::JiraCloud), &q(1, None)).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)), "{e}");
    assert!(m.requests().is_empty());
}
