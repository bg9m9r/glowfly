//! Account SSH keys against a mock server (see `mock`): the requests each service gets, the
//! duplicate check, errors that call for the copy-and-open fallback, and the pure helpers.

use super::super::mock::{Mock, Reply, Request};
use super::*;

const KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGada ada@laptop";

fn account(provider: Provider, host: &str) -> Account {
    Account { provider, host: host.into(), org: String::new() }
}

fn token(provider: Provider) -> Auth {
    Auth::from_token(provider, None, "tok", "test")
}

fn routes(m: &Mock) -> Vec<String> {
    m.requests().iter().map(Request::route).collect()
}

fn posted(m: &Mock) -> Request {
    let posts: Vec<Request> = m.requests().into_iter().filter(|r| r.method == "POST").collect();
    assert_eq!(posts.len(), 1, "{:?}", routes(m));
    posts.into_iter().next().unwrap()
}

#[test]
fn github_lists_every_page_then_adds() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user/keys?per_page=100" => Reply::json(r#"[{"id":1,"key":"ssh-rsa AAAAB3other","title":"old"}]"#)
            .header("Link", r#"<https://api.github.com/user/keys?per_page=100&page=2>; rel="next""#),
        "GET api.github.com/user/keys?per_page=100&page=2" => Reply::json(r#"[{"id":2,"key":"ssh-ed25519 AAAAzzz","title":"work"}]"#),
        "POST api.github.com/user/keys" => Reply::json(r#"{"id":3,"key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGada","title":"laptop"}"#),
        _ => Reply::not_found(),
    });
    let gh = account(Provider::GitHub, "github.com");
    let out = add_ssh_key(&gh, &token(Provider::GitHub), KEY, "laptop").unwrap();
    assert_eq!(out, AddKeyOutcome::Added { title: "laptop".into() });
    assert_eq!(
        routes(&m),
        ["GET api.github.com/user/keys?per_page=100", "GET api.github.com/user/keys?per_page=100&page=2", "POST api.github.com/user/keys"]
    );
    let post = posted(&m);
    assert_eq!(post.header("Authorization"), Some("Bearer tok"));
    assert_eq!(post.json(), serde_json::json!({"title": "laptop", "key": KEY}));
    let listed = list_ssh_keys(&gh, &token(Provider::GitHub)).unwrap();
    assert_eq!(listed.iter().map(|k| k.title.as_str()).collect::<Vec<_>>(), ["old", "work"]);
}

#[test]
fn a_key_already_there_is_not_posted() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user/keys?per_page=100" => {
            Reply::json(r#"[{"id":7,"key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGada","title":"Laptop \u001b[2Jkey"}]"#)
        }
        _ => Reply::not_found(),
    });
    let out = add_ssh_key(&account(Provider::GitHub, "github.com"), &token(Provider::GitHub), KEY, "new").unwrap();
    assert_eq!(out, AddKeyOutcome::AlreadyThere { title: "Laptop [2Jkey".into() }, "the comment doesn't count; the title is cleaned");
    assert_eq!(routes(&m), ["GET api.github.com/user/keys?per_page=100"]);
}

#[test]
fn enterprise_server_uses_its_own_api() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET git.acme.io/api/v3/user/keys?per_page=100" => Reply::json("[]"),
        "POST git.acme.io/api/v3/user/keys" => Reply::json(r#"{"id":1,"key":"ssh-ed25519 AAAA","title":"t"}"#),
        _ => Reply::not_found(),
    });
    let ghes = account(Provider::GitHubEnterprise, "git.acme.io");
    assert_eq!(add_ssh_key(&ghes, &token(Provider::GitHubEnterprise), KEY, "t").unwrap(), AddKeyOutcome::Added { title: "t".into() });
    assert_eq!(routes(&m).len(), 2);
}

#[test]
fn gitlab_lists_and_adds() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/user/keys?per_page=100" => Reply::json(r#"[{"id":1,"title":"old","key":"ssh-rsa AAAAB3"}]"#),
        "POST gitlab.com/api/v4/user/keys" => Reply::json(r#"{"id":2,"title":"laptop","key":"ssh-ed25519 AAAA"}"#),
        _ => Reply::not_found(),
    });
    let gl = account(Provider::GitLab, "gitlab.com");
    assert_eq!(add_ssh_key(&gl, &token(Provider::GitLab), KEY, "laptop").unwrap(), AddKeyOutcome::Added { title: "laptop".into() });
    assert_eq!(posted(&m).json(), serde_json::json!({"title": "laptop", "key": KEY}));
}

#[test]
fn errors_decide_the_fallback() {
    let gl = account(Provider::GitLab, "gitlab.com");
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/user/keys?per_page=100" => Reply::json("[]"),
        _ => Reply::status(400, r#"{"message":{"key":["has already been taken"]}}"#),
    });
    let e = add_ssh_key(&gl, &token(Provider::GitLab), KEY, "t").unwrap_err();
    assert!(matches!(e, Error::Http { status: 400, .. }) && !needs_fallback(&e), "{e}");
    drop(_m);

    let gh = account(Provider::GitHub, "github.com");
    let _m = Mock::start(|_| Reply::not_found());
    let e = add_ssh_key(&gh, &token(Provider::GitHub), KEY, "t").unwrap_err();
    assert!(needs_fallback(&e), "a 404 on the list: {e}");
    drop(_m);

    let _m = Mock::start(|r| match r.method.as_str() {
        "GET" => Reply::json("[]"),
        _ => Reply::status(403, r#"{"message":"Resource not accessible by integration"}"#),
    });
    let e = add_ssh_key(&gh, &token(Provider::GitHub), KEY, "t").unwrap_err();
    assert!(matches!(e, Error::Forbidden(_)) && needs_fallback(&e), "{e}");
    assert!(!needs_fallback(&Error::Auth("401".into())));
    assert!(!needs_fallback(&Error::Network("down".into())));
}

#[test]
fn bitbucket_cloud_adds_to_the_users_keys() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"uuid":"{abc}","display_name":"Ada"}"#),
        "GET api.bitbucket.org/2.0/users/%7Babc%7D/ssh-keys?pagelen=100" => Reply::json(r#"{"values":[]}"#),
        "POST api.bitbucket.org/2.0/users/%7Babc%7D/ssh-keys" => Reply::json(r#"{"uuid":"{k}","key":"ssh-ed25519 AAAA","label":"laptop"}"#),
        _ => Reply::not_found(),
    });
    let bb = account(Provider::Bitbucket, "bitbucket.org");
    assert_eq!(add_ssh_key(&bb, &token(Provider::Bitbucket), KEY, "laptop").unwrap(), AddKeyOutcome::Added { title: "laptop".into() });
    assert_eq!(posted(&m).json(), serde_json::json!({"key": KEY, "label": "laptop"}));
}

#[test]
fn other_services_send_nothing() {
    let m = Mock::start(|_| Reply::json("[]"));
    for p in [Provider::BitbucketDataCenter, Provider::AzureDevOps, Provider::JiraCloud] {
        let a = Account { provider: p, host: "h.example".into(), org: "o".into() };
        let e = add_ssh_key(&a, &token(p), KEY, "t").unwrap_err();
        assert!(matches!(e, Error::Unsupported(_)) && needs_fallback(&e), "{p:?}: {e}");
        assert!(list_ssh_keys(&a, &token(p)).is_err());
    }
    assert!(m.requests().is_empty());
}

#[test]
fn settings_pages() {
    let page = |p: Provider, host: &str, org: &str| ssh_keys_page(&Account { provider: p, host: host.into(), org: org.into() });
    assert_eq!(page(Provider::GitHub, "github.com", "").as_deref(), Some("https://github.com/settings/keys"));
    assert_eq!(page(Provider::GitHubEnterprise, "git.acme.io", "").as_deref(), Some("https://git.acme.io/settings/keys"));
    assert_eq!(page(Provider::GitLab, "gitlab.com", "").as_deref(), Some("https://gitlab.com/-/profile/keys"));
    assert_eq!(page(Provider::GitLabSelfManaged, "gl.acme.io", "").as_deref(), Some("https://gl.acme.io/-/profile/keys"));
    assert_eq!(page(Provider::Bitbucket, "bitbucket.org", "").as_deref(), Some("https://bitbucket.org/account/settings/ssh-keys/"));
    assert_eq!(
        page(Provider::BitbucketDataCenter, "bb.acme.io", "").as_deref(),
        Some("https://bb.acme.io/plugins/servlet/ssh/account/keys")
    );
    assert_eq!(page(Provider::AzureDevOps, "dev.azure.com", "acme").as_deref(), Some("https://dev.azure.com/acme/_usersSettings/keys"));
    assert_eq!(page(Provider::AzureDevOps, "dev.azure.com", ""), None);
    assert_eq!(page(Provider::JiraCloud, "acme.atlassian.net", ""), None);
    assert_eq!(page(Provider::JiraDataCenter, "jira.acme.io", ""), None);
}

#[test]
fn titles() {
    assert_eq!(default_title("ada@laptop", "id_ed25519"), "ada@laptop");
    assert_eq!(default_title("", "id_ed25519"), "id_ed25519");
    assert_eq!(default_title("  \u{1b}  ", "id_work"), "id_work");
    assert_eq!(default_title(&"x".repeat(150), "id").chars().count(), 100);
    assert_eq!(title_problem("a\nb"), Some("The title can't contain line breaks or control characters."));
    assert_eq!(title_problem("work laptop"), None);
    assert!(same_key(KEY, "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGada"));
    assert!(!same_key(KEY, "ssh-rsa AAAAC3NzaC1lZDI1NTE5AAAAIGada"));
    assert!(!same_key("junk", "junk"));
}
