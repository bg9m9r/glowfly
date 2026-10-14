//! Lists longer than one page, for every service's paging style.

use super::*;
use crate::forge::page::MAX_ITEMS;

fn gh_issue(n: usize) -> String {
    format!(r#"{{"number":{n},"title":"I{n}","state":"open","html_url":"https://github.com/o/r/issues/{n}"}}"#)
}

fn gh_pr(n: usize) -> String {
    format!(
        r#"{{"number":{n},"title":"P{n}","html_url":"https://github.com/o/r/pull/{n}","user":{{"login":"a"}},
        "head":{{"ref":"b{n}","sha":""}},"base":{{"ref":"main","sha":""}}}}"#
    )
}

fn array(items: impl Iterator<Item = String>) -> String {
    format!("[{}]", items.collect::<Vec<_>>().join(","))
}

fn page_of(path: &str) -> usize {
    let n = path.split(['?', '&']).find_map(|p| p.strip_prefix("page="));
    n.and_then(|n| n.parse().ok()).unwrap_or(1)
}

#[test]
fn github_follows_next_links_on_the_api_host_only() {
    let m = Mock::start(|r| {
        let route = r.route();
        if route.starts_with("GET api.github.com/repos/o/r/pulls?") {
            match page_of(&r.path) {
                1 => Reply::json(array((1..=2).map(gh_pr)))
                    .header("Link", r#"<https://api.github.com/repositories/1/pulls?state=open&per_page=100&page=2>; rel="next""#),
                _ => Reply::not_found(),
            }
        } else if route.starts_with("GET api.github.com/repositories/1/pulls?") {
            // The last page points somewhere else; that is never followed.
            Reply::json(array((3..=4).map(gh_pr))).header("Link", r#"<https://evil.example/steal?page=3>; rel="next""#)
        } else if route == "POST api.github.com/graphql" {
            Reply::json(r#"{"data":{"repository":{}}}"#)
        } else {
            Reply::not_found()
        }
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert!(routes(&m).iter().all(|r| r.split_once(' ').is_some_and(|(_, url)| url.starts_with("api.github.com/"))), "{:?}", routes(&m));
    // The token went to the second page too.
    assert_eq!(find(&m, "GET api.github.com/repositories/1/pulls?").header("Authorization"), Some("Bearer tok"));
}

#[test]
fn github_lists_stop_at_the_item_cap() {
    let m = Mock::start(|r| {
        if r.route() == "GET api.github.com/user" {
            return Reply::json(r#"{"login":"ada"}"#);
        }
        let page = page_of(&r.path);
        let next = format!(r#"<https://api.github.com/repos/o/r/issues?assignee=ada&page={}>; rel="next""#, page + 1);
        Reply::json(array((1..=100).map(|i| gh_issue((page - 1) * 100 + i)))).header("Link", &next)
    });
    let issues = my_issues(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!((issues.len(), issues.last().unwrap().key.as_str()), (MAX_ITEMS, "1000"));
    assert_eq!(routes(&m).len(), 1 + MAX_ITEMS / 100);
}

#[test]
fn gitlab_follows_next_links() {
    let mr = |n: usize| {
        format!(
            r#"{{"iid":{n},"title":"M{n}","source_branch":"b","target_branch":"main","author":{{"name":"A"}},
            "web_url":"https://gitlab.com/g/app/-/merge_requests/{n}"}}"#
        )
    };
    let m = Mock::start(move |r| {
        let route = r.route();
        if route.contains("/approvals") {
            Reply::json("{}")
        } else if route.starts_with("GET gitlab.com/api/v4/projects/g%2Fapp/merge_requests?") {
            let page = page_of(&r.path);
            let reply = Reply::json(array([page * 10 + 1, page * 10 + 2].into_iter().map(mr)));
            if page < 3 {
                let next = format!(
                    r#"<https://gitlab.com/api/v4/projects/g%2Fapp/merge_requests?state=opened&per_page=100&page={}>; rel="next""#,
                    page + 1
                );
                reply.header("Link", &next)
            } else {
                reply
            }
        } else {
            Reply::not_found()
        }
    });
    let prs = pull_requests(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab)).unwrap();
    assert_eq!(prs.iter().map(|p| p.id).collect::<Vec<_>>(), [11, 12, 21, 22, 31, 32]);
    assert_eq!(routes(&m).iter().filter(|r| r.contains("merge_requests?")).count(), 3);
}

#[test]
fn bitbucket_cloud_follows_next_and_data_center_next_page_start() {
    let pr = |n: usize| {
        format!(
            r#"{{"id":{n},"title":"P","source":{{"branch":{{"name":"b"}}}},"destination":{{"branch":{{"name":"main"}}}},
            "links":{{"html":{{"href":"https://bitbucket.org/t/a/pull-requests/{n}"}}}}}}"#
        )
    };
    let _m = Mock::start(move |r| match page_of(&r.path) {
        1 => Reply::json(format!(
            r#"{{"values":[{}],"next":"https://api.bitbucket.org/2.0/repositories/t/a/pullrequests?state=OPEN&page=2"}}"#,
            pr(1)
        )),
        2 => Reply::json(format!(r#"{{"values":[{}],"next":"https://bitbucket.evil.example/x?page=3"}}"#, pr(2))),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://bitbucket.org/t/a"), &token(Provider::Bitbucket)).unwrap();
    assert_eq!(prs.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2]);
    drop(_m);

    let dc = |n: usize| {
        format!(
            r#"{{"id":{n},"title":"P","fromRef":{{"displayId":"b"}},"toRef":{{"displayId":"main"}},"author":{{"user":{{}}}},"links":{{}}}}"#
        )
    };
    let m = Mock::start(move |r| {
        if r.path.ends_with("&start=25") {
            Reply::json(format!(r#"{{"isLastPage":true,"values":[{}]}}"#, dc(26)))
        } else {
            Reply::json(format!(r#"{{"isLastPage":false,"nextPageStart":25,"values":[{}]}}"#, dc(1)))
        }
    });
    let t = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    let prs = pull_requests(&t, &token(Provider::BitbucketDataCenter)).unwrap();
    assert_eq!(prs.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 26]);
    assert_eq!(
        routes(&m).last().unwrap(),
        "GET bitbucket.acme.io/rest/api/1.0/projects/PROJ/repos/api/pull-requests?state=OPEN&limit=100&start=25"
    );
}

fn az_pr(n: usize) -> String {
    format!(r#"{{"pullRequestId":{n},"title":"P","sourceRefName":"refs/heads/b","targetRefName":"refs/heads/main"}}"#)
}

#[test]
fn azure_skips_full_pages_and_honours_continuation_tokens() {
    // Full pages are followed with $skip until a short page.
    let m = Mock::start(|r| {
        let (skip, count) = if r.path.contains("$skip=100") { (100, 30) } else { (0, 100) };
        Reply::json(format!(r#"{{"value":{}}}"#, array((skip + 1..=skip + count).map(az_pr))))
    });
    let t = remote("https://dev.azure.com/org/proj/_git/app");
    let prs = pull_requests(&t, &token(Provider::AzureDevOps)).unwrap();
    assert_eq!((prs.len(), prs[129].id), (130, 130));
    assert_eq!(routes(&m).len(), 2);
    drop(m);

    // A continuation token, when sent, is passed back instead.
    let m = Mock::start(|r| {
        if r.path.contains("continuationToken=a%2Bb") {
            Reply::json(format!(r#"{{"value":[{}]}}"#, az_pr(2)))
        } else {
            Reply::json(format!(r#"{{"value":[{}]}}"#, az_pr(1))).header("x-ms-continuationtoken", "a+b")
        }
    });
    let prs = pull_requests(&t, &token(Provider::AzureDevOps)).unwrap();
    assert_eq!(prs.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(routes(&m).len(), 2);
}

fn jira_issue(key: &str) -> String {
    format!(r#"{{"key":"{key}","fields":{{"summary":"S"}}}}"#)
}

#[test]
fn jira_pages_by_token_on_cloud_and_by_start_on_data_center() {
    let m = Mock::start(|r| {
        if r.path.contains("nextPageToken=t2") {
            Reply::json(format!(r#"{{"issues":[{}],"isLast":true}}"#, jira_issue("A-2")))
        } else {
            Reply::json(format!(r#"{{"issues":[{}],"nextPageToken":"t2"}}"#, jira_issue("A-1")))
        }
    });
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let found = my_issues(&Target::jira(&account, ""), &token(Provider::JiraCloud)).unwrap();
    assert_eq!(found.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["A-1", "A-2"]);
    assert!(routes(&m)[0].starts_with("GET acme.atlassian.net/rest/api/3/search/jql?"));
    drop(m);

    let m = Mock::start(|r| {
        if r.path.ends_with("&startAt=1") {
            Reply::json(format!(r#"{{"startAt":1,"total":2,"issues":[{}]}}"#, jira_issue("B-2")))
        } else {
            Reply::json(format!(r#"{{"startAt":0,"total":2,"issues":[{}]}}"#, jira_issue("B-1")))
        }
    });
    let account = Account { provider: Provider::JiraDataCenter, host: "jira.acme.io".into(), org: String::new() };
    let found = my_issues(&Target::jira(&account, "B"), &token(Provider::JiraDataCenter)).unwrap();
    assert_eq!(found.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["B-1", "B-2"]);
    assert_eq!(routes(&m).len(), 2);
}
