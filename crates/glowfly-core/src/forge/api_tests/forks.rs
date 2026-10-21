use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn quick(attempts: u32) -> Wait {
    Wait { attempts, interval: std::time::Duration::ZERO }
}

fn gh_repo(owner: &str, private: bool, html: &str) -> String {
    format!(
        r#"{{"owner":{{"login":"{owner}"}},"full_name":"{owner}/r","clone_url":"https://github.com/{owner}/r.git",
        "ssh_url":"git@github.com:{owner}/r.git","html_url":"{html}","private":{private}}}"#
    )
}

#[test]
fn github_lists_forks_across_pages() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/forks?per_page=100&sort=newest" => {
            Reply::json(format!("[{}]", gh_repo("ada", false, "https://github.com/ada/r")))
                .header("Link", r#"<https://api.github.com/repositories/1/forks?page=2>; rel="next""#)
        }
        "GET api.github.com/repositories/1/forks?page=2" => {
            Reply::json(format!("[{}]", gh_repo("eve", true, "https://evil.example/eve/r")))
        }
        _ => Reply::not_found(),
    });
    let list = list_forks(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert!(!list.truncated);
    let ada = &list.forks[0];
    assert_eq!((ada.owner.as_str(), ada.full_name.as_str(), ada.private), ("ada", "ada/r", false));
    assert_eq!((ada.https_url.as_str(), ada.ssh_url.as_str()), ("https://github.com/ada/r.git", "git@github.com:ada/r.git"));
    assert_eq!(ada.web_url, "https://github.com/ada/r");
    let eve = &list.forks[1];
    assert_eq!((eve.full_name.as_str(), eve.private, eve.web_url.as_str()), ("eve/r", true, ""), "off-site link emptied");
    assert_eq!(find(&m, "GET api.github.com/repos/o/r/forks").header("Authorization"), Some("Bearer tok"));
}

#[test]
fn github_enterprise_uses_its_own_api() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET git.acme.io/api/v3/repos/o/r/forks?per_page=100&sort=newest" => Reply::json("[]"),
        _ => Reply::not_found(),
    });
    let t = Target {
        provider: Provider::GitHubEnterprise,
        host: "git.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    assert!(list_forks(&t, &token(Provider::GitHubEnterprise)).unwrap().forks.is_empty());
    assert_eq!(routes(&m).len(), 1);
}

#[test]
fn github_finds_the_users_fork() {
    let fork = r#"{"owner":{"login":"ada"},"full_name":"ada/r","clone_url":"https://github.com/ada/r.git","ssh_url":"",
        "html_url":"https://github.com/ada/r","fork":true,"parent":{"full_name":"O/R"}}"#;
    let m = Mock::start(move |r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada"}"#),
        "GET api.github.com/repos/ada/r" => Reply::json(fork),
        _ => Reply::not_found(),
    });
    let t = remote("https://github.com/o/r");
    let found = find_my_fork(&t, &token(Provider::GitHub)).unwrap();
    assert_eq!((found.user.as_str(), found.own_repository), ("ada", false));
    assert_eq!(found.mine.unwrap().full_name, "ada/r");
    assert_eq!(routes(&m), ["GET api.github.com/user", "GET api.github.com/repos/ada/r"]);
    drop(m);

    // Renamed: not at ada/r, so the newest forks are scanned for ada's.
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"ada"}"#),
        "GET api.github.com/repos/o/r/forks?per_page=100&sort=newest" => {
            Reply::json(format!("[{},{}]", gh_repo("grace", false, ""), gh_repo("Ada", false, "")))
        }
        _ => Reply::not_found(),
    });
    let found = find_my_fork(&t, &token(Provider::GitHub)).unwrap();
    assert_eq!(found.mine.map(|f| f.owner), Some("Ada".into()));

    // The user's own repository: nothing to look up.
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/user" => Reply::json(r#"{"login":"O"}"#),
        _ => Reply::not_found(),
    });
    let found = find_my_fork(&t, &token(Provider::GitHub)).unwrap();
    assert!(found.own_repository && found.mine.is_none());
    assert_eq!(routes(&m), ["GET api.github.com/user"]);
}

/// A server that creates ada/r and answers its lookups with `statuses` in turn (then 404s).
fn create_server(statuses: &'static [u16]) -> Mock {
    let seen = Arc::new(AtomicUsize::new(0));
    Mock::start(move |r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/forks" => Reply::status(202, gh_repo("ada", false, "https://github.com/ada/r")),
        "GET api.github.com/repos/ada/r" => match statuses.get(seen.fetch_add(1, Ordering::SeqCst)) {
            Some(200) => Reply::json(gh_repo("ada", false, "")),
            _ => Reply::not_found(),
        },
        _ => Reply::status(500, "unexpected"),
    })
}

#[test]
fn github_creates_a_fork_and_waits_for_it() {
    let t = remote("https://github.com/o/r");
    let m = create_server(&[404, 404, 200]);
    let created = create_fork(&t, &token(Provider::GitHub), &quick(3)).unwrap();
    assert!(created.ready);
    assert_eq!(created.fork.https_url, "https://github.com/ada/r.git");
    assert_eq!(find(&m, "POST api.github.com/repos/o/r/forks").json(), serde_json::json!({}));
    assert_eq!(routes(&m).iter().filter(|r| r.as_str() == "GET api.github.com/repos/ada/r").count(), 3);
    drop(m);

    let _m = create_server(&[]);
    assert!(!create_fork(&t, &token(Provider::GitHub), &quick(2)).unwrap().ready, "never ready within the wait");

    let _m = Mock::start(|_| Reply::status(422, r#"{"message":"Validation Failed"}"#));
    let e = create_fork(&t, &token(Provider::GitHub), &quick(1)).unwrap_err();
    assert!(matches!(&e, Error::Http { status: 422, message } if message.contains("Validation Failed")), "{e}");
}

fn gl_project(id: u64, ns: &str, visibility: &str, status: &str) -> String {
    format!(
        r#"{{"id":{id},"path_with_namespace":"{ns}/app","namespace":{{"full_path":"{ns}"}},
        "http_url_to_repo":"https://gitlab.com/{ns}/app.git","ssh_url_to_repo":"git@gitlab.com:{ns}/app.git",
        "web_url":"https://gitlab.com/{ns}/app","visibility":"{visibility}","import_status":"{status}"}}"#
    )
}

#[test]
fn gitlab_lists_and_finds_forks() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects/g%2Fsub%2Fapp/forks?per_page=100&order_by=created_at&sort=desc" => {
            Reply::json(format!("[{},{}]", gl_project(1, "ada", "public", "none"), gl_project(2, "team/x", "private", "none")))
        }
        "GET gitlab.com/api/v4/user" => Reply::json(r#"{"username":"ada","name":"Ada"}"#),
        "GET gitlab.com/api/v4/projects/g%2Fsub%2Fapp/forks?owned=true&per_page=100" => {
            Reply::json(format!("[{},{}]", gl_project(2, "team/x", "private", "none"), gl_project(1, "ADA", "public", "none")))
        }
        _ => Reply::not_found(),
    });
    let t = remote("git@gitlab.com:g/sub/app.git");
    let list = list_forks(&t, &token(Provider::GitLab)).unwrap();
    let owners: Vec<(&str, bool)> = list.forks.iter().map(|f| (f.owner.as_str(), f.private)).collect();
    assert_eq!(owners, [("ada", false), ("team/x", true)]);
    assert_eq!(list.forks[0].ssh_url, "git@gitlab.com:ada/app.git");
    let found = find_my_fork(&t, &token(Provider::GitLab)).unwrap();
    assert_eq!((found.user.as_str(), found.own_repository), ("ada", false));
    assert_eq!(found.mine.map(|f| f.full_name), Some("ADA/app".into()), "the user's namespace first");
    assert_eq!(routes(&m).len(), 3);
}

#[test]
fn gitlab_creates_a_fork_and_waits_for_the_import() {
    let seen = Arc::new(AtomicUsize::new(0));
    let m = Mock::start(move |r| match r.route().as_str() {
        "POST gitlab.com/api/v4/projects/g%2Fapp/fork" => Reply::status(201, gl_project(42, "ada", "public", "scheduled")),
        "GET gitlab.com/api/v4/projects/42" => {
            let status = if seen.fetch_add(1, Ordering::SeqCst) == 0 { "started" } else { "finished" };
            Reply::json(gl_project(42, "ada", "public", status))
        }
        _ => Reply::not_found(),
    });
    let t = remote("https://gitlab.com/g/app");
    let created = create_fork(&t, &token(Provider::GitLab), &quick(3)).unwrap();
    assert!(created.ready);
    assert_eq!(created.fork.full_name, "ada/app");
    assert_eq!(routes(&m).len(), 3);
    drop(m);

    let _m = Mock::start(|r| match r.route().as_str() {
        "POST gitlab.com/api/v4/projects/g%2Fapp/fork" => Reply::status(201, gl_project(42, "ada", "public", "scheduled")),
        _ => Reply::json(gl_project(42, "ada", "public", "failed")),
    });
    let e = create_fork(&t, &token(Provider::GitLab), &quick(3)).unwrap_err();
    assert!(e.to_string().contains("import failed"), "{e}");
}

#[test]
fn bitbucket_cloud_lists_forks() {
    let fork = |ws: &str| {
        format!(
            r#"{{"full_name":"{ws}/app","is_private":true,"workspace":{{"slug":"{ws}"}},"links":{{
            "clone":[{{"name":"https","href":"https://ada@bitbucket.org/{ws}/app.git"}},{{"name":"ssh","href":"git@bitbucket.org:{ws}/app.git"}}],
            "html":{{"href":"https://bitbucket.org/{ws}/app"}}}}}}"#
        )
    };
    let (one, two) = (fork("ada"), fork("grace"));
    let _m = Mock::start(move |r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/ws/app/forks?pagelen=100" => Reply::json(format!(
            r#"{{"values":[{one}],"next":"https://api.bitbucket.org/2.0/repositories/ws/app/forks?pagelen=100&page=2"}}"#
        )),
        "GET api.bitbucket.org/2.0/repositories/ws/app/forks?pagelen=100&page=2" => Reply::json(format!(r#"{{"values":[{two}]}}"#)),
        _ => Reply::not_found(),
    });
    let list = list_forks(&remote("https://bitbucket.org/ws/app.git"), &token(Provider::Bitbucket)).unwrap();
    assert_eq!(list.forks.len(), 2);
    let f = &list.forks[0];
    assert_eq!((f.owner.as_str(), f.full_name.as_str(), f.private), ("ada", "ada/app", true));
    assert_eq!((f.https_url.as_str(), f.ssh_url.as_str()), ("https://bitbucket.org/ada/app.git", "git@bitbucket.org:ada/app.git"));
}

#[test]
fn bitbucket_data_center_lists_forks_by_page_start() {
    let personal = r#"{"slug":"app","public":false,"project":{"key":"~ADA","type":"PERSONAL","owner":{"slug":"ada"}},
        "links":{"clone":[{"name":"http","href":"https://ada@bb.acme.io/scm/~ada/app.git"},{"name":"ssh","href":"ssh://git@bb.acme.io:7999/~ada/app.git"}],
        "self":[{"href":"https://bb.acme.io/users/ada/repos/app/browse"}]}}"#;
    let project = r#"{"slug":"app-copy","public":true,"project":{"key":"OPS","type":"NORMAL"},
        "links":{"clone":[{"name":"http","href":"https://bb.acme.io/scm/ops/app-copy.git"}],"self":[]}}"#;
    let _m = Mock::start(move |r| match r.route().as_str() {
        "GET bb.acme.io/rest/api/1.0/projects/PROJ/repos/app/forks?limit=100" => {
            Reply::json(format!(r#"{{"values":[{personal}],"isLastPage":false,"nextPageStart":1}}"#))
        }
        "GET bb.acme.io/rest/api/1.0/projects/PROJ/repos/app/forks?limit=100&start=1" => {
            Reply::json(format!(r#"{{"values":[{project}],"isLastPage":true}}"#))
        }
        _ => Reply::not_found(),
    });
    let t = remote("https://bb.acme.io/scm/PROJ/app.git");
    let list = list_forks(&t, &token(Provider::BitbucketDataCenter)).unwrap();
    let rows: Vec<(&str, &str, bool)> = list.forks.iter().map(|f| (f.owner.as_str(), f.full_name.as_str(), f.private)).collect();
    assert_eq!(rows, [("ada", "~ADA/app", true), ("OPS", "OPS/app-copy", false)]);
    assert_eq!(list.forks[0].https_url, "https://bb.acme.io/scm/~ada/app.git");
    assert_eq!(list.forks[0].ssh_url, "ssh://git@bb.acme.io:7999/~ada/app.git");
}

#[test]
fn unsupported_services_send_nothing() {
    let m = Mock::start(|_| Reply::json("{}"));
    let bb = remote("https://bitbucket.org/ws/app");
    let auth = token(Provider::Bitbucket);
    assert!(matches!(find_my_fork(&bb, &auth), Err(Error::Unsupported(_))));
    assert!(matches!(create_fork(&bb, &auth, &quick(1)), Err(Error::Unsupported(_))));
    let az = remote("https://dev.azure.com/org/proj/_git/app");
    assert!(matches!(list_forks(&az, &token(Provider::AzureDevOps)), Err(Error::Unsupported(_))));
    assert!(routes(&m).is_empty());
}
