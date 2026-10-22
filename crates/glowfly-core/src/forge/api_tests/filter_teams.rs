//! The teams and groups read with the signed-in user, and team review requests on pull requests.

use super::*;
use crate::forge::filters;

const GH_USER: &str = "GET api.github.com/user";
const GH_TEAMS: &str = "GET api.github.com/user/teams?per_page=100";
const AZ_ME: &str = "GET dev.azure.com/contoso/_apis/connectionData?api-version=7.1-preview";
const AZ_MEMBERSHIP: &str = "GET vssps.dev.azure.com/contoso/_apis/identities?identityIds=a-1&queryMembership=expanded&api-version=7.1";
const AZ_DESCRIPTORS: &str = "GET vssps.dev.azure.com/contoso/_apis/identities?descriptors=";

fn azure() -> Target {
    remote("https://dev.azure.com/contoso/p/_git/r")
}

#[test]
fn github_reads_teams_with_the_user() {
    let m = Mock::start(|r| match r.route().as_str() {
        GH_USER => Reply::json(r#"{"login":"ada","name":"Ada"}"#),
        GH_TEAMS => Reply::json(
            r#"[{"slug":"core","organization":{"login":"acme"},"parent":{"slug":"eng"}},{"slug":"eng","organization":{"login":"acme"}},
            {"slug":"docs","organization":{"login":"other"}}]"#,
        ),
        _ => Reply::not_found(),
    });
    let (t, auth) = (remote("https://github.com/o/r"), token(Provider::GitHub));
    let me = filters::me(&t, &auth).unwrap();
    assert_eq!(me.teams, ["acme/core", "acme/eng", "other/docs"], "a parent team is added once");
    assert_eq!(filters::my_teams(&t, &auth, &me).unwrap(), me.teams);
    assert!(m.requests().iter().all(|r| r.header("Authorization").is_some()), "{:?}", routes(&m));
}

#[test]
fn github_team_lookup_failure_keeps_the_user() {
    for (status, body) in [
        (403, r#"{"message":"Resource not accessible by personal access token"}"#),
        (401, r#"{"message":"Bad credentials"}"#),
        (404, r#"{"message":"Not Found"}"#),
        (500, "<html>server error</html>"),
        (200, "<html>sign in</html>"),
    ] {
        let _m = Mock::start(move |r| match r.route().as_str() {
            GH_USER => Reply::json(r#"{"login":"ada","name":"Ada"}"#),
            _ if status == 200 => Reply::text(body),
            _ => Reply::status(status, body),
        });
        let (t, auth) = (remote("https://github.com/o/r"), token(Provider::GitHub));
        let me = filters::me(&t, &auth).unwrap();
        assert_eq!((me.id.as_str(), me.teams.len()), ("ada", 0), "{status}");
        assert!(filters::my_teams(&t, &auth, &me).is_err(), "{status}");
    }
}

#[test]
fn github_enterprise_reads_teams_on_its_api() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET github.acme.io/api/v3/user" => Reply::json(r#"{"login":"ada","name":"Ada"}"#),
        "GET github.acme.io/api/v3/user/teams?per_page=100" => Reply::json(r#"[{"slug":"core","organization":{"login":"acme"}}]"#),
        _ => Reply::not_found(),
    });
    let t = remote("https://github.acme.io/o/r");
    assert_eq!(t.provider, Provider::GitHubEnterprise);
    let me = filters::me(&t, &token(Provider::GitHubEnterprise)).unwrap();
    assert_eq!(me.teams, ["acme/core"], "{:?}", routes(&m));
}

#[test]
fn github_pull_requests_carry_requested_teams() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r/pulls?state=open&per_page=100" => Reply::json(
            r#"[{"number":4,"title":"T","html_url":"https://github.com/o/r/pull/4","user":{"login":"grace"},"head":{"ref":"x","sha":""},
            "base":{"ref":"main","sha":"","user":{"login":"o"}},"requested_teams":[{"slug":"core"}]}]"#,
        ),
        _ => Reply::not_found(),
    });
    let prs = pull_requests(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(prs[0].facets.awaiting_team_ids, ["o/core"]);
    assert!(prs[0].facets.reviewer_ids.is_empty());
}

#[test]
fn azure_reads_groups_through_identities() {
    let m = Mock::start(|r| {
        let route = r.route();
        if route == AZ_ME {
            Reply::json(r#"{"authenticatedUser":{"id":"a-1","providerDisplayName":"Ada"}}"#)
        } else if route == AZ_MEMBERSHIP {
            Reply::json(
                r#"{"count":1,"value":[{"id":"a-1","memberOf":["Microsoft.TeamFoundation.Identity;S-1-9-1",
                {"identityType":"Microsoft.TeamFoundation.Identity","identifier":"S-1-9-2"}]}]}"#,
            )
        } else if route.starts_with(AZ_DESCRIPTORS) {
            Reply::json(r#"{"value":[{"id":"g-1"},null,{"id":"t-1"}]}"#)
        } else {
            Reply::not_found()
        }
    });
    let me = filters::me(&azure(), &token(Provider::AzureDevOps)).unwrap();
    assert_eq!((me.id.as_str(), me.teams.as_slice()), ("a-1", &["g-1".to_owned(), "t-1".into()][..]));
    let path = find(&m, AZ_DESCRIPTORS).path;
    let (one, two) = (enc("Microsoft.TeamFoundation.Identity;S-1-9-1"), enc("Microsoft.TeamFoundation.Identity;S-1-9-2"));
    assert!(path.contains(&format!("descriptors={one},{two}&queryMembership=none")), "{path}");
    assert_eq!(routes(&m).len(), 3);
}

#[test]
fn azure_group_lookup_failure_keeps_the_user() {
    for html in [false, true] {
        let _m = Mock::start(move |r| match r.route().as_str() {
            AZ_ME => Reply::json(r#"{"authenticatedUser":{"id":"a-1","providerDisplayName":"Ada"}}"#),
            _ if html => Reply::text("<html>sign in</html>"),
            _ => Reply::status(401, r#"{"message":"TF400813: not authorized"}"#),
        });
        let me = filters::me(&azure(), &token(Provider::AzureDevOps)).unwrap();
        assert_eq!((me.id.as_str(), me.teams.len()), ("a-1", 0), "html: {html}");
    }
}

#[test]
fn services_without_team_requests_ask_nothing_more() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/user" => Reply::json(r#"{"name":"Ada L","username":"ada"}"#),
        "GET api.bitbucket.org/2.0/user" => Reply::json(r#"{"display_name":"Ada","uuid":"{u-1}"}"#),
        _ => Reply::not_found(),
    });
    assert!(filters::me(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab)).unwrap().teams.is_empty());
    assert_eq!(routes(&m).len(), 1);
    assert!(filters::me(&remote("https://bitbucket.org/t/a"), &token(Provider::Bitbucket)).unwrap().teams.is_empty());
    assert_eq!(routes(&m).len(), 2);
}
