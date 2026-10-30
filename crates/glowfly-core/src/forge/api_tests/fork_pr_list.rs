//! Fork pull requests in each service's list (`PullRequest::head_repo`): a branch of the same
//! repository, one on a fork (named as the service names it), and one whose fork was deleted,
//! in the shapes the services answer with. Every other call is refused, so only the list counts.

use super::*;

/// A mock that answers `list` (a route) with `body` and refuses everything else.
fn list_server(list: String, body: &'static str) -> Mock {
    Mock::start(move |r| if r.route() == list { Reply::json(body) } else { Reply::not_found() })
}

fn heads(prs: &[PullRequest]) -> Vec<(u64, &str, HeadRepo)> {
    prs.iter().map(|p| (p.id, p.source.as_str(), p.head_repo.clone())).collect()
}

fn fork(name: &str) -> HeadRepo {
    HeadRepo::Fork { name: name.into() }
}

/// GitHub's `head.repo` is null once the fork is deleted; `base.repo` is always there.
const GH: &str = r#"[
  {"number":1,"title":"Own","html_url":"https://github.com/team/app/pull/1","user":{"login":"ada"},
   "head":{"ref":"topic","sha":"a1","repo":{"full_name":"Team/App"}},"base":{"ref":"main","sha":"b","repo":{"full_name":"team/app"}}},
  {"number":2,"title":"Fork","html_url":"https://github.com/team/app/pull/2","user":{"login":"bob"},
   "head":{"ref":"topic","sha":"a2","repo":{"full_name":"bob/app-fork"}},"base":{"ref":"main","sha":"b","repo":{"full_name":"team/app"}}},
  {"number":3,"title":"Gone","html_url":"https://github.com/team/app/pull/3","user":{"login":"eve"},
   "head":{"ref":"topic","sha":"a3","repo":null},"base":{"ref":"main","sha":"b","repo":{"full_name":"team/app"}}}
]"#;

#[test]
fn github_lists_the_head_repository() {
    let _m = list_server("GET api.github.com/repos/team/app/pulls?state=open&per_page=100".into(), GH);
    let base = remote("https://github.com/team/app");
    let prs = pull_requests(&base, &token(Provider::GitHub)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", HeadRepo::Same), (2, "topic", fork("bob/app-fork")), (3, "topic", HeadRepo::Deleted)]);
    // The fork's own remote is its source; the base repository's same-named branch isn't.
    assert!(prs[1].head_repo.is(&remote("git@github.com:bob/app-fork.git"), &base));
    assert!(!prs[1].head_repo.is(&base, &base));
}

#[test]
fn github_enterprise_server_lists_the_head_repository() {
    let _m = list_server("GET github.acme.io/api/v3/repos/team/app/pulls?state=open&per_page=100".into(), GH);
    let base = remote("https://github.acme.io/team/app.git");
    let prs = pull_requests(&base, &token(Provider::GitHubEnterprise)).unwrap();
    assert_eq!(prs[1].head_repo, fork("bob/app-fork"));
    assert!(prs[1].head_repo.is(&remote("ssh://git@github.acme.io/Bob/app-fork"), &base));
}

/// GitLab names the source project by id only (its path is read by id); a deleted fork's
/// `source_project_id` is null.
const GL: &str = r#"[
  {"iid":1,"title":"Own","source_branch":"topic","target_branch":"main","author":{"name":"Ada"},
   "web_url":"https://gitlab.com/team/app/-/merge_requests/1","source_project_id":7,"target_project_id":7},
  {"iid":2,"title":"Fork","source_branch":"topic","target_branch":"main","author":{"name":"Bob"},
   "web_url":"https://gitlab.com/team/app/-/merge_requests/2","source_project_id":9,"target_project_id":7},
  {"iid":3,"title":"Gone","source_branch":"topic","target_branch":"main","author":{"name":"Eve"},
   "web_url":"https://gitlab.com/team/app/-/merge_requests/3","source_project_id":null,"target_project_id":7}
]"#;

/// Two merge requests from the same fork project, so its path is asked once.
const GL_TWO: &str = r#"[
  {"iid":2,"title":"Fork","source_branch":"topic","target_branch":"main","author":{"name":"Bob"},
   "web_url":"https://gitlab.com/team/app/-/merge_requests/2","source_project_id":9,"target_project_id":7},
  {"iid":4,"title":"Fork too","source_branch":"other","target_branch":"main","author":{"name":"Bob"},
   "web_url":"https://gitlab.com/team/app/-/merge_requests/4","source_project_id":9,"target_project_id":7}
]"#;

#[test]
fn gitlab_reads_a_forks_path_by_its_project_id() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects/team%2Fapp/merge_requests?state=opened&per_page=100" => Reply::json(GL_TWO),
        "GET gitlab.com/api/v4/projects/9" => Reply::json(r#"{"id":9,"path_with_namespace":"bob/sub/app"}"#),
        _ => Reply::not_found(),
    });
    let base = remote("https://gitlab.com/team/app");
    let prs = pull_requests(&base, &token(Provider::GitLab)).unwrap();
    assert_eq!(heads(&prs), [(2, "topic", fork("bob/sub/app")), (4, "other", fork("bob/sub/app"))]);
    assert_eq!(routes(&m).iter().filter(|r| r.as_str() == "GET gitlab.com/api/v4/projects/9").count(), 1, "asked once");
    assert!(prs[0].head_repo.is(&remote("git@gitlab.com:Bob/sub/app.git"), &base));
    assert!(!prs[0].head_repo.is(&base, &base));
}

#[test]
fn gitlab_lists_forks_by_project_id() {
    // The fork's project can't be read here: it stays unknown, and the list still loads.
    let _m = list_server("GET gitlab.com/api/v4/projects/team%2Fapp/merge_requests?state=opened&per_page=100".into(), GL);
    let base = remote("https://gitlab.com/team/app");
    let prs = pull_requests(&base, &token(Provider::GitLab)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", HeadRepo::Same), (2, "topic", fork("")), (3, "topic", HeadRepo::Deleted)]);
    // An unknown fork is no local remote's, not even the base repository's.
    assert!(!prs[1].head_repo.is(&base, &base));
}

#[test]
fn gitlab_self_managed_reads_the_forks_path_on_its_own_server() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.acme.io/api/v4/projects/group%2Fapp/merge_requests?state=opened&per_page=100" => Reply::json(GL),
        "GET gitlab.acme.io/api/v4/projects/9" => Reply::json(r#"{"path_with_namespace":"ada/app"}"#),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://gitlab.acme.io/group/app"), &token(Provider::GitLabSelfManaged)).unwrap();
    assert_eq!(prs[1].head_repo, fork("ada/app"));
    assert_eq!(find(&m, "GET gitlab.acme.io/api/v4/projects/9").header("Authorization"), Some("Bearer tok"));
}

/// Entries missing a repository's name read as an unknown fork; the rest of the list loads.
#[test]
fn a_nameless_head_repository_is_unknown_not_a_failed_list() {
    const GH_ODD: &str = r#"[
      {"number":1,"title":"Odd","html_url":"https://github.com/team/app/pull/1","user":{"login":"ada"},
       "head":{"ref":"topic","sha":"a1","repo":{}},"base":{"ref":"main","sha":"b","repo":{"full_name":"team/app"}}},
      {"number":2,"title":"Own","html_url":"https://github.com/team/app/pull/2","user":{"login":"ada"},
       "head":{"ref":"x","sha":"a2","repo":{"full_name":"team/app"}},"base":{"ref":"main","sha":"b","repo":{"full_name":"team/app"}}}
    ]"#;
    let _m = list_server("GET api.github.com/repos/team/app/pulls?state=open&per_page=100".into(), GH_ODD);
    let prs = pull_requests(&remote("https://github.com/team/app"), &token(Provider::GitHub)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", fork("")), (2, "x", HeadRepo::Same)]);
    drop(_m);

    const AZ_ODD: &str = r#"{"count":2,"value":[
      {"pullRequestId":1,"title":"Odd","sourceRefName":"refs/heads/topic","targetRefName":"refs/heads/main","createdBy":{"displayName":"Ada"},
       "forkSource":{"repository":{"id":"f1","name":"web-fork"}}},
      {"pullRequestId":2,"title":"Own","sourceRefName":"refs/heads/x","targetRefName":"refs/heads/main","createdBy":{"displayName":"Ada"}}
    ]}"#;
    let list =
        "GET dev.azure.com/contoso/Web/_apis/git/repositories/app/pullrequests?searchCriteria.status=active&$top=100&api-version=7.1";
    let _m = list_server(list.into(), AZ_ODD);
    let prs = pull_requests(&remote("https://dev.azure.com/contoso/Web/_git/app"), &token(Provider::AzureDevOps)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", fork("")), (2, "x", HeadRepo::Same)]);
    drop(_m);

    const BB_ODD: &str = r#"{"values":[
      {"id":1,"title":"Odd","source":{"branch":{"name":"topic"},"repository":{"type":"repository"}},
       "destination":{"branch":{"name":"main"},"repository":{"full_name":"team/app"}},"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/1"}}}
    ]}"#;
    let list = "GET api.bitbucket.org/2.0/repositories/team/app/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants";
    let _m = list_server(list.into(), BB_ODD);
    let prs = pull_requests(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", fork(""))]);
    drop(_m);

    const DC_ODD: &str = r#"{"isLastPage":true,"values":[
      {"id":1,"title":"Odd","fromRef":{"displayId":"topic","repository":{"slug":"api"}},
       "toRef":{"displayId":"master","repository":{"slug":"api","project":{"key":"PROJ"}}},"author":{"user":{"displayName":"Ada"}},
       "links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/1"}]}}
    ]}"#;
    let _m = list_server("GET bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests?state=OPEN&limit=100".into(), DC_ODD);
    let prs = pull_requests(&remote("https://bitbucket.acme.io/scm/PROJ/api.git"), &token(Provider::BitbucketDataCenter)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", fork(""))]);
}

/// Bitbucket Cloud's `source.repository` is null once the fork is deleted.
const BB: &str = r#"{"values":[
  {"id":1,"title":"Own","source":{"branch":{"name":"topic"},"repository":{"full_name":"team/app"}},
   "destination":{"branch":{"name":"main"},"repository":{"full_name":"team/app"}},"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/1"}}},
  {"id":2,"title":"Fork","source":{"branch":{"name":"topic"},"repository":{"full_name":"bob/app"}},
   "destination":{"branch":{"name":"main"},"repository":{"full_name":"team/app"}},"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/2"}}},
  {"id":3,"title":"Gone","source":{"branch":{"name":"topic"},"repository":null},
   "destination":{"branch":{"name":"main"},"repository":{"full_name":"team/app"}},"links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/3"}}}
]}"#;

#[test]
fn bitbucket_cloud_lists_the_source_repository() {
    let list = "GET api.bitbucket.org/2.0/repositories/team/app/pullrequests?state=OPEN&pagelen=50&fields=%2Bvalues.participants";
    let _m = list_server(list.into(), BB);
    let base = remote("https://bitbucket.org/team/app");
    let prs = pull_requests(&base, &token(Provider::Bitbucket)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", HeadRepo::Same), (2, "topic", fork("bob/app")), (3, "topic", HeadRepo::Deleted)]);
    assert!(prs[1].head_repo.is(&remote("git@bitbucket.org:Bob/app.git"), &base));
}

/// Data Center gives each ref's repository by project key and slug; a deleted fork's is missing.
const DC: &str = r#"{"isLastPage":true,"values":[
  {"id":1,"title":"Own","fromRef":{"displayId":"topic","repository":{"slug":"api","project":{"key":"PROJ"}}},
   "toRef":{"displayId":"master","repository":{"slug":"api","project":{"key":"PROJ"}}},"author":{"user":{"displayName":"Ada"}},
   "links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/1"}]}},
  {"id":2,"title":"Fork","fromRef":{"displayId":"topic","repository":{"slug":"api","project":{"key":"~BOB"}}},
   "toRef":{"displayId":"master","repository":{"slug":"api","project":{"key":"PROJ"}}},"author":{"user":{"displayName":"Bob"}},
   "links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/2"}]}},
  {"id":3,"title":"Gone","fromRef":{"displayId":"topic"},
   "toRef":{"displayId":"master","repository":{"slug":"api","project":{"key":"PROJ"}}},"author":{"user":{"displayName":"Eve"}},
   "links":{"self":[{"href":"https://bitbucket.acme.io/projects/PROJ/repos/api/pull-requests/3"}]}}
]}"#;

#[test]
fn bitbucket_data_center_lists_the_from_repository() {
    let _m = list_server("GET bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests?state=OPEN&limit=100".into(), DC);
    let base = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    let prs = pull_requests(&base, &token(Provider::BitbucketDataCenter)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", HeadRepo::Same), (2, "topic", fork("~BOB/api")), (3, "topic", HeadRepo::Deleted)]);
    assert!(prs[1].head_repo.is(&remote("ssh://git@bitbucket.acme.io:7999/~bob/api.git"), &base));
    assert!(!prs[1].head_repo.is(&base, &base));
}

/// Azure DevOps sets `forkSource` only for a fork; its repository is missing once it's deleted.
const AZ: &str = r#"{"count":3,"value":[
  {"pullRequestId":1,"title":"Own","sourceRefName":"refs/heads/topic","targetRefName":"refs/heads/main","createdBy":{"displayName":"Ada"}},
  {"pullRequestId":2,"title":"Fork","sourceRefName":"refs/heads/topic","targetRefName":"refs/heads/main","createdBy":{"displayName":"Bob"},
   "forkSource":{"name":"refs/heads/topic","repository":{"id":"f1","name":"web-fork","project":{"id":"p2","name":"Forks"}}}},
  {"pullRequestId":3,"title":"Gone","sourceRefName":"refs/heads/topic","targetRefName":"refs/heads/main","createdBy":{"displayName":"Eve"},
   "forkSource":{"name":"refs/heads/topic"}}
]}"#;

#[test]
fn azure_devops_lists_the_fork_source() {
    let base_api = "dev.azure.com/contoso/Web/_apis/git/repositories/app";
    let _m = list_server(format!("GET {base_api}/pullrequests?searchCriteria.status=active&$top=100&api-version=7.1"), AZ);
    let base = remote("https://dev.azure.com/contoso/Web/_git/app");
    let prs = pull_requests(&base, &token(Provider::AzureDevOps)).unwrap();
    assert_eq!(heads(&prs), [(1, "topic", HeadRepo::Same), (2, "topic", fork("Forks/web-fork")), (3, "topic", HeadRepo::Deleted)]);
    assert!(prs[1].head_repo.is(&remote("https://contoso@dev.azure.com/contoso/Forks/_git/web-fork"), &base));
}
