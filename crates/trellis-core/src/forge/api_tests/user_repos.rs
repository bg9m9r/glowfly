use super::*;

fn account(provider: Provider, host: &str) -> Account {
    Account { provider, host: host.into(), org: String::new() }
}

fn gh_repo(full: &str, private: bool, https: &str, ssh: &str, html: &str) -> String {
    let owner = full.split('/').next().unwrap();
    format!(
        r#"{{"owner":{{"login":"{owner}"}},"full_name":"{full}","clone_url":"{https}","ssh_url":"{ssh}","html_url":"{html}","private":{private}}}"#
    )
}

const GH_FIRST: &str = "GET api.github.com/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member";

#[test]
fn github_lists_the_users_repositories_across_pages() {
    let m = Mock::start(|r| match r.route().as_str() {
        GH_FIRST => Reply::json(format!(
            "[{},{}]",
            gh_repo("ada/app", false, "https://github.com/ada/app.git", "git@github.com:ada/app.git", "https://github.com/ada/app"),
            gh_repo("team/api", true, "https://github.com/team/api.git", "git@github.com:team/api.git", "https://evil.example/x"),
        ))
        .header("Link", r#"<https://api.github.com/user/repos?page=2>; rel="next""#),
        "GET api.github.com/user/repos?page=2" => Reply::json(format!(
            "[{}]",
            gh_repo("eve/bad", false, "https://evil.example/eve/bad.git", "git@evil.example:eve/bad.git", "https://github.com/eve/bad")
        )),
        _ => Reply::not_found(),
    });
    let list = my_repos(&account(Provider::GitHub, "github.com"), &token(Provider::GitHub)).unwrap();
    assert!(!list.truncated);
    let names: Vec<&str> = list.repos.iter().map(|r| r.full_name.as_str()).collect();
    assert_eq!(names, ["ada/app", "team/api"], "a repository with no usable clone URL is dropped");
    let ada = &list.repos[0];
    assert_eq!((ada.owner.as_str(), ada.private), ("ada", false));
    assert_eq!((ada.https_url.as_str(), ada.ssh_url.as_str()), ("https://github.com/ada/app.git", "git@github.com:ada/app.git"));
    let team = &list.repos[1];
    assert!(team.private);
    assert_eq!(team.web_url, "", "an off-site web link is emptied");
    assert_eq!(find(&m, GH_FIRST).header("Authorization"), Some("Bearer tok"));
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn github_enterprise_and_gitlab_use_their_hosts() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET git.acme.io/api/v3/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member" => {
            Reply::json("[]")
        }
        _ => Reply::not_found(),
    });
    assert!(my_repos(&account(Provider::GitHubEnterprise, "git.acme.io"), &token(Provider::GitHubEnterprise)).unwrap().repos.is_empty());
    assert_eq!(routes(&m).len(), 1);
    drop(m);

    let project = |path: &str, visibility: &str, host: &str| {
        let ns = path.rsplit_once('/').unwrap().0;
        format!(
            r#"{{"path_with_namespace":"{path}","namespace":{{"full_path":"{ns}"}},"http_url_to_repo":"https://{host}/{path}.git",
            "ssh_url_to_repo":"git@{host}:{path}.git","web_url":"https://{host}/{path}","visibility":"{visibility}"}}"#
        )
    };
    let body = format!("[{},{}]", project("g/sub/app", "public", "gitlab.com"), project("ada/notes", "internal", "gitlab.com"));
    let m = Mock::start(move |r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects?membership=true&per_page=100&order_by=last_activity_at&sort=desc" => Reply::json(body.clone()),
        _ => Reply::not_found(),
    });
    let list = my_repos(&account(Provider::GitLab, "gitlab.com"), &token(Provider::GitLab)).unwrap();
    let got: Vec<(&str, &str, bool)> = list.repos.iter().map(|r| (r.owner.as_str(), r.full_name.as_str(), r.private)).collect();
    assert_eq!(got, [("g/sub", "g/sub/app", false), ("ada", "ada/notes", true)]);
    assert_eq!(list.repos[0].ssh_url, "git@gitlab.com:g/sub/app.git");
    assert_eq!(routes(&m).len(), 1);
    drop(m);

    let body = format!("[{}]", project("ops/infra", "private", "git.corp.io"));
    let m = Mock::start(move |r| match r.route().as_str() {
        "GET git.corp.io/api/v4/projects?membership=true&per_page=100&order_by=last_activity_at&sort=desc" => Reply::json(body.clone()),
        _ => Reply::not_found(),
    });
    let list = my_repos(&account(Provider::GitLabSelfManaged, "git.corp.io"), &token(Provider::GitLabSelfManaged)).unwrap();
    assert_eq!(list.repos[0].https_url, "https://git.corp.io/ops/infra.git");
    assert_eq!(routes(&m).len(), 1);
}

fn bb_repo(ws: &str, name: &str, updated: &str, private: bool) -> String {
    format!(
        r#"{{"full_name":"{ws}/{name}","is_private":{private},"workspace":{{"slug":"{ws}"}},"updated_on":"{updated}",
        "links":{{"clone":[{{"name":"https","href":"https://ada@bitbucket.org/{ws}/{name}.git"}},{{"name":"ssh","href":"git@bitbucket.org:{ws}/{name}.git"}}],
        "html":{{"href":"https://bitbucket.org/{ws}/{name}"}}}}}}"#
    )
}

fn bitbucket_server(workspaces: &'static str, fallback: bool) -> Mock {
    let one = bb_repo("ada", "old", "2026-01-01T10:00:00.000000+00:00", false);
    let two = bb_repo("ada", "new", "2026-03-01T10:00:00.000000+00:00", true);
    let three = bb_repo("team", "mid", "2026-02-01T10:00:00.000000+00:00", false);
    Mock::start(move |r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/user/workspaces?pagelen=100" if fallback => Reply::not_found(),
        "GET api.bitbucket.org/2.0/user/workspaces?pagelen=100" | "GET api.bitbucket.org/2.0/user/permissions/workspaces?pagelen=100" => {
            Reply::json(format!(r#"{{"values":{workspaces}}}"#))
        }
        "GET api.bitbucket.org/2.0/repositories/ada?pagelen=100&sort=-updated_on" => Reply::json(format!(r#"{{"values":[{two},{one}]}}"#)),
        "GET api.bitbucket.org/2.0/repositories/team?pagelen=100&sort=-updated_on" => Reply::json(format!(r#"{{"values":[{three}]}}"#)),
        _ => Reply::not_found(),
    })
}

#[test]
fn bitbucket_merges_every_workspaces_repositories() {
    let m = bitbucket_server(r#"[{"workspace":{"slug":"ada"}},{"workspace":{"slug":"team"}}]"#, false);
    let list = my_repos(&account(Provider::Bitbucket, "bitbucket.org"), &token(Provider::Bitbucket)).unwrap();
    let names: Vec<&str> = list.repos.iter().map(|r| r.full_name.as_str()).collect();
    assert_eq!(names, ["ada/new", "team/mid", "ada/old"], "newest update first");
    assert_eq!(list.repos[0].https_url, "https://bitbucket.org/ada/new.git", "the user@ part is stripped");
    assert_eq!(list.repos[0].ssh_url, "git@bitbucket.org:ada/new.git");
    assert!(list.repos[0].private && !list.repos[1].private);
    assert_eq!(routes(&m).len(), 3);
    drop(m);

    // Without /user/workspaces, the permissions list; plain workspace entries parse too.
    let m = bitbucket_server(r#"[{"slug":"team"}]"#, true);
    let list = my_repos(&account(Provider::Bitbucket, "bitbucket.org"), &token(Provider::Bitbucket)).unwrap();
    assert_eq!(list.repos.iter().map(|r| r.full_name.as_str()).collect::<Vec<_>>(), ["team/mid"]);
    assert_eq!(
        routes(&m),
        [
            "GET api.bitbucket.org/2.0/user/workspaces?pagelen=100",
            "GET api.bitbucket.org/2.0/user/permissions/workspaces?pagelen=100",
            "GET api.bitbucket.org/2.0/repositories/team?pagelen=100&sort=-updated_on",
        ]
    );
}

#[test]
fn other_services_are_unsupported_without_a_request() {
    let m = Mock::start(|_| Reply::json("[]"));
    for (p, host) in
        [(Provider::AzureDevOps, "dev.azure.com"), (Provider::BitbucketDataCenter, "bb.corp.io"), (Provider::JiraCloud, "x.atlassian.net")]
    {
        assert!(matches!(my_repos(&account(p, host), &token(p)), Err(Error::Unsupported(_))), "{p:?}");
        assert!(!lists_my_repos(p));
    }
    assert!(m.requests().is_empty());
    for p in [Provider::GitHub, Provider::GitHubEnterprise, Provider::GitLab, Provider::GitLabSelfManaged, Provider::Bitbucket] {
        assert!(lists_my_repos(p), "{p:?}");
    }
    assert!(!lists_my_repos(Provider::JiraDataCenter));
}

#[test]
fn a_full_list_is_marked_truncated() {
    let page: Vec<String> = (0..100)
        .map(|i| {
            let full = format!("ada/r{i}");
            gh_repo(&full, false, &format!("https://github.com/{full}.git"), "", "")
        })
        .collect();
    let body = format!("[{}]", page.join(","));
    let _m = Mock::start(move |r| {
        let n: usize = r.path.rsplit_once("page=").and_then(|(_, n)| n.parse().ok()).unwrap_or(1);
        Reply::json(body.clone()).header("Link", &format!(r#"<https://api.github.com/user/repos?page={}>; rel="next""#, n + 1))
    });
    let list = my_repos(&account(Provider::GitHub, "github.com"), &token(Provider::GitHub)).unwrap();
    assert!(list.truncated);
    assert_eq!(list.repos.len(), 1000);
}
