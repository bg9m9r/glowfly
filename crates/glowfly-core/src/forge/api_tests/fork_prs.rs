//! Pull requests from a branch on a fork: each service is asked to open the pull request on the
//! target repository with the head on the fork (`NewPullRequest::source_repo`).

use super::*;
use serde_json::json;

fn new_pr(source_repo: Option<Target>) -> NewPullRequest {
    NewPullRequest {
        source: "topic".into(),
        target: "main".into(),
        title: "New".into(),
        description: "Body".into(),
        draft: false,
        issues: Vec::new(),
        source_repo,
    }
}

const GH_PR: &str = r#"{"number":12,"title":"New","html_url":"https://github.com/team/app/pull/12","user":{"login":"ada"},
  "head":{"ref":"topic","sha":"0123456789"},"base":{"ref":"main","sha":"y"}}"#;
const GL_MR: &str = r#"{"iid":8,"title":"New","source_branch":"topic","target_branch":"main","author":{"name":"Ada"},
  "web_url":"https://gitlab.com/team/app/-/merge_requests/8"}"#;
const BB_PR: &str = r#"{"id":20,"title":"New","source":{"branch":{"name":"topic"}},"destination":{"branch":{"name":"main"}},
  "links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/20"}}}"#;
const DC_PR: &str = r#"{"id":3,"title":"New","fromRef":{"displayId":"topic"},"toRef":{"displayId":"main"},
  "author":{"user":{"displayName":"Ada"}},"links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/3"}]}}"#;
const AZ_PR: &str = r#"{"pullRequestId":17,"title":"New","sourceRefName":"refs/heads/topic","targetRefName":"refs/heads/main",
  "createdBy":{"displayName":"Ada"}}"#;

/// Answers every POST with `created` and every GET with `got`.
fn server(created: &'static str, got: &'static str) -> Mock {
    Mock::start(move |r| if r.method == "POST" { Reply::status(201, created) } else { Reply::json(got) })
}

#[test]
fn github_sends_the_fork_owner_in_head() {
    let m = server(GH_PR, "{}");
    let pr = new_pr(Some(remote("https://github.com/ada/app")));
    create_pull_request(&remote("https://github.com/team/app"), &token(Provider::GitHub), &pr).unwrap();
    assert_eq!(routes(&m), ["POST api.github.com/repos/team/app/pulls"]);
    let sent = m.requests()[0].json();
    assert_eq!((sent["head"].as_str(), sent["base"].as_str()), (Some("ada:topic"), Some("main")));
    assert!(sent.get("head_repo").is_none(), "{sent}");
}

#[test]
fn github_names_the_head_repository_for_a_fork_with_the_same_owner() {
    let m = server(GH_PR, "{}");
    let pr = new_pr(Some(remote("https://github.com/team/app-fork")));
    create_pull_request(&remote("https://github.com/Team/app"), &token(Provider::GitHub), &pr).unwrap();
    let sent = find(&m, "POST api.github.com/repos/Team/app/pulls").json();
    assert_eq!((sent["head"].as_str(), sent["head_repo"].as_str()), (Some("team:topic"), Some("app-fork")));
}

#[test]
fn github_enterprise_server_fork() {
    let m = server(GH_PR, "{}");
    let pr = new_pr(Some(remote("https://github.acme.io/ada/app.git")));
    create_pull_request(&remote("https://github.acme.io/team/app.git"), &token(Provider::GitHubEnterprise), &pr).unwrap();
    assert_eq!(routes(&m), ["POST github.acme.io/api/v3/repos/team/app/pulls"]);
    assert_eq!(m.requests()[0].json()["head"], "ada:topic");
}

#[test]
fn gitlab_posts_on_the_fork_with_the_target_project_id() {
    let m = server(GL_MR, r#"{"id":42}"#);
    let pr = new_pr(Some(remote("https://gitlab.com/ada/app")));
    create_pull_request(&remote("https://gitlab.com/team/app"), &token(Provider::GitLab), &pr).unwrap();
    assert_eq!(routes(&m), ["GET gitlab.com/api/v4/projects/team%2Fapp", "POST gitlab.com/api/v4/projects/ada%2Fapp/merge_requests"]);
    let sent = m.requests()[1].json();
    assert_eq!(sent["target_project_id"], 42);
    assert_eq!((sent["source_branch"].as_str(), sent["target_branch"].as_str()), (Some("topic"), Some("main")));

    // Without the target project's id nothing is created.
    let m = Mock::start(|_| Reply::not_found());
    let e = create_pull_request(&remote("https://gitlab.com/team/app"), &token(Provider::GitLab), &pr).unwrap_err();
    assert!(e.is_not_found(), "{e}");
    assert_eq!(routes(&m), ["GET gitlab.com/api/v4/projects/team%2Fapp"]);
}

#[test]
fn bitbucket_cloud_names_the_fork_in_source() {
    let m = server(BB_PR, "{}");
    let pr = new_pr(Some(remote("https://bitbucket.org/ada/app")));
    create_pull_request(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket), &pr).unwrap();
    assert_eq!(routes(&m), ["POST api.bitbucket.org/2.0/repositories/team/app/pullrequests"]);
    let sent = m.requests()[0].json();
    assert_eq!(sent["source"], json!({"branch":{"name":"topic"},"repository":{"full_name":"ada/app"}}));
    assert_eq!(sent["destination"]["branch"]["name"], "main");
}

#[test]
fn bitbucket_data_center_names_a_personal_fork_in_from_ref() {
    let m = server(DC_PR, "{}");
    let pr = new_pr(Some(remote("https://bitbucket.acme.io/scm/~ada/api.git")));
    create_pull_request(&remote("https://bitbucket.acme.io/scm/PROJ/api.git"), &token(Provider::BitbucketDataCenter), &pr).unwrap();
    assert_eq!(routes(&m), ["POST bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests"]);
    let sent = m.requests()[0].json();
    assert_eq!(sent["fromRef"]["repository"], json!({"slug":"api","project":{"key":"~ada"}}));
    assert_eq!(sent["toRef"]["repository"], json!({"slug":"api","project":{"key":"PROJ"}}));
}

#[test]
fn azure_devops_sends_the_fork_repository_id() {
    let m = server(AZ_PR, r#"{"id":"f0rk","defaultBranch":"refs/heads/main"}"#);
    let pr = new_pr(Some(remote("https://dev.azure.com/org/proj/_git/app-fork")));
    create_pull_request(&remote("https://dev.azure.com/org/proj/_git/app"), &token(Provider::AzureDevOps), &pr).unwrap();
    assert_eq!(
        routes(&m),
        [
            "GET dev.azure.com/org/proj/_apis/git/repositories/app-fork?api-version=7.1",
            "POST dev.azure.com/org/proj/_apis/git/repositories/app/pullrequests?api-version=7.1",
        ]
    );
    let sent = m.requests()[1].json();
    assert_eq!(sent["forkSource"], json!({"repository":{"id":"f0rk"}}));
    assert_eq!(sent["sourceRefName"], "refs/heads/topic");
}

#[test]
fn a_source_repository_equal_to_the_target_sends_todays_body() {
    let cases: [(&str, &str, Provider, &str); 5] = [
        ("https://github.com/team/app", "https://github.com/TEAM/App", Provider::GitHub, GH_PR),
        ("https://gitlab.com/team/app", "https://gitlab.com/Team/APP", Provider::GitLab, GL_MR),
        ("https://bitbucket.org/team/app", "https://bitbucket.org/team/App", Provider::Bitbucket, BB_PR),
        ("https://bitbucket.acme.io/scm/PROJ/api.git", "https://bitbucket.acme.io/scm/proj/API.git", Provider::BitbucketDataCenter, DC_PR),
        ("https://dev.azure.com/org/proj/_git/app", "https://dev.azure.com/org/Proj/_git/App", Provider::AzureDevOps, AZ_PR),
    ];
    for (url, same, provider, reply) in cases {
        let plain = server(reply, "{}");
        create_pull_request(&remote(url), &token(provider), &new_pr(None)).unwrap();
        let (plain_routes, plain_body) = (routes(&plain), plain.requests()[0].json());
        drop(plain);
        let m = server(reply, "{}");
        create_pull_request(&remote(url), &token(provider), &new_pr(Some(remote(same)))).unwrap();
        assert_eq!((routes(&m), m.requests()[0].json()), (plain_routes, plain_body), "{url}");
    }
}

#[test]
fn a_source_repository_on_another_account_is_refused_before_any_request() {
    let m = server(GH_PR, "{}");
    let gh = remote("https://github.com/team/app");
    let e = create_pull_request(&gh, &token(Provider::GitHub), &new_pr(Some(remote("https://gitlab.com/ada/app")))).unwrap_err();
    assert!(e.to_string().contains("same service account"), "{e}");
    let az = remote("https://dev.azure.com/org/proj/_git/app");
    let other_org = remote("https://dev.azure.com/other/proj/_git/app");
    assert!(create_pull_request(&az, &token(Provider::AzureDevOps), &new_pr(Some(other_org))).is_err());
    assert!(routes(&m).is_empty());
}
