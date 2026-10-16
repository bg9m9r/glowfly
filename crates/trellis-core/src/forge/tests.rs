use super::*;

fn t(p: Provider, host: &str, owner: &str, repo: &str) -> Target {
    Target { provider: p, host: host.into(), owner: owner.into(), project: String::new(), repo: repo.into() }
}

#[test]
fn detects_hosts() {
    assert_eq!(parse_remote("https://github.com/morluto/rea", &[]), Some(t(Provider::GitHub, "github.com", "morluto", "rea")));
    assert_eq!(parse_remote("git@github.com:owner/repo.git", &[]), Some(t(Provider::GitHub, "github.com", "owner", "repo")));
    assert_eq!(parse_remote("https://gitlab.com/group/sub/proj.git", &[]), Some(t(Provider::GitLab, "gitlab.com", "group/sub", "proj")));
    assert_eq!(parse_remote("git@bitbucket.org:team/app.git", &[]), Some(t(Provider::Bitbucket, "bitbucket.org", "team", "app")));
    assert_eq!(
        parse_remote("https://bitbucket.acme.io/scm/PROJ/api.git", &[]),
        Some(t(Provider::BitbucketDataCenter, "bitbucket.acme.io", "PROJ", "api"))
    );
    assert_eq!(
        parse_remote("https://gitlab.acme.io/team/api.git", &[]),
        Some(t(Provider::GitLabSelfManaged, "gitlab.acme.io", "team", "api"))
    );
    assert_eq!(
        parse_remote("https://github.acme.io/team/api.git", &[]),
        Some(t(Provider::GitHubEnterprise, "github.acme.io", "team", "api"))
    );
    // Unknown hosts are recognized once connected in Preferences.
    assert_eq!(parse_remote("https://code.acme.io/team/api.git", &[]), None);
    let conn = Connection {
        account: Account { provider: Provider::GitHubEnterprise, host: "code.acme.io".into(), org: String::new() },
        user: "u".into(),
        method: "m".into(),
    };
    assert_eq!(parse_remote("git@code.acme.io:team/api.git", &[conn]), Some(t(Provider::GitHubEnterprise, "code.acme.io", "team", "api")));
    let az = parse_remote("https://dev.azure.com/contoso/Web/_git/app", &[]).unwrap();
    assert_eq!((az.provider, az.owner.as_str(), az.project.as_str(), az.repo.as_str()), (Provider::AzureDevOps, "contoso", "Web", "app"));
}

/// Read-only check against a real service, using whatever credentials Trellis would find.
/// Run with: TRELLIS_LIVE_REMOTE=<remote url> cargo test live_remote -- --ignored --nocapture
#[test]
#[ignore]
fn live_remote() {
    let url = std::env::var("TRELLIS_LIVE_REMOTE").expect("set TRELLIS_LIVE_REMOTE");
    let t = parse_remote(&url, &[]).expect("unrecognized remote");
    let account = t.account();
    let auth = resolve_auth(&account, &crate::paths::temp_dir(), false, true).expect("no credentials");
    println!("{} {} via {}", t.provider.name(), t.display(), auth.source);
    println!("user: {}", verify(&account, &auth).expect("verify"));
    println!("default branch: {:?}", default_branch(&t, &auth).expect("default branch"));
    let prs = pull_requests(&t, &auth).expect("pull requests");
    println!("{} open pull requests", prs.len());
    for p in prs.iter().take(5) {
        println!("  #{} {} ({} -> {}) approvals={}", p.id, p.title, p.source, p.target, p.approvals());
    }
    let issues = my_issues(&t, &auth).expect("issues");
    println!("{} assigned issues", issues.len());
    for i in issues.iter().take(5) {
        println!("  #{} {} [{}]", i.key, i.title, i.state);
    }
}

#[test]
fn parses_host_fields() {
    assert_eq!(parse_host(Provider::GitHub, ""), Some(("github.com".into(), String::new())));
    assert_eq!(parse_host(Provider::JiraCloud, "https://acme.atlassian.net/jira"), Some(("acme.atlassian.net".into(), String::new())));
    assert_eq!(parse_host(Provider::GitLabSelfManaged, "gitlab"), None);
    assert_eq!(parse_host(Provider::AzureDevOps, "dev.azure.com/acme"), Some(("dev.azure.com".into(), "acme".into())));
}

#[test]
fn finds_issue_references() {
    assert_eq!(issue_refs(RefStyle::AzureBoards, "Fix AB#123, ab#45; LAB#9 and #7"), ["123", "45"]);
    // Cross-repository references (owner/repo#3) point elsewhere and are skipped.
    assert_eq!(issue_refs(RefStyle::Hash, "Fixes #12 and owner/repo#3, not &#39; or a#5"), ["12"]);
    // UTF-8 has the shape of a Jira key; lookups skip keys that don't exist, and a project
    // filter (see ForgeState::issue_refs) narrows matches further.
    assert_eq!(issue_refs(RefStyle::Jira, "PROJ-12: fix ABC-7 in UTF-8 (X-1 too short)"), ["PROJ-12", "ABC-7", "UTF-8"]);
    assert_eq!(branch_issue(RefStyle::Hash, "feature/1234-login"), Some("1234".into()));
    assert_eq!(branch_issue(RefStyle::Jira, "bugfix/PROJ-12-crash"), Some("PROJ-12".into()));
    assert_eq!(branch_issue(RefStyle::Jira, "feature/login-PROJ-12"), None);
    assert_eq!(branch_issue(RefStyle::Hash, "release/2024.1"), None);
    let issue = Issue {
        key: "PROJ-12".into(),
        title: "Crash on start!".into(),
        state: String::new(),
        kind: String::new(),
        assignee: None,
        url: String::new(),
    };
    assert_eq!(branch_name_for(&issue), "PROJ-12-crash-on-start");
    assert_eq!(link_line(Provider::GitHub, Provider::GitHub, "5").as_deref(), Some("Closes #5"));
    assert_eq!(link_line(Provider::AzureDevOps, Provider::AzureDevOps, "5"), None);
    assert_eq!(link_line(Provider::BitbucketDataCenter, Provider::JiraDataCenter, "AB-1").as_deref(), Some("AB-1"));
}

#[test]
fn issue_links_encode_owner_repo_and_key() {
    let t = parse_remote("https://gitlab.com/group/%2E%2E/sub/proj", &[]).unwrap();
    assert_eq!(t.issue_url("7"), "https://gitlab.com/group/%2E%2E/sub/proj/-/issues/7");
    let t = parse_remote("https://bitbucket.org/..%2Fteam/app", &[]).unwrap();
    assert_eq!(t.issue_url("3?x"), "https://bitbucket.org/..%2Fteam/app/issues/3%3Fx");
}

#[test]
fn service_links_and_names_per_provider() {
    let acct = |provider, host: &str, org: &str| Account { provider, host: host.into(), org: org.into() };
    let token = |a: Account| Target::token_url(&a);
    assert_eq!(token(acct(Provider::GitHubEnterprise, "git.acme.io", "")).as_deref(), Some("https://git.acme.io/settings/tokens"));
    assert_eq!(
        token(acct(Provider::GitLab, "gitlab.com", "")).as_deref(),
        Some("https://gitlab.com/-/user_settings/personal_access_tokens")
    );
    assert_eq!(
        token(acct(Provider::Bitbucket, "bitbucket.org", "")).as_deref(),
        Some("https://bitbucket.org/account/settings/app-passwords/")
    );
    assert_eq!(
        token(acct(Provider::BitbucketDataCenter, "bb.acme.io", "")).as_deref(),
        Some("https://bb.acme.io/plugins/servlet/access-tokens/manage")
    );
    assert_eq!(
        token(acct(Provider::AzureDevOps, "dev.azure.com", "my co")).as_deref(),
        Some("https://dev.azure.com/my%20co/_usersSettings/tokens")
    );
    assert_eq!(token(acct(Provider::AzureDevOps, "dev.azure.com", "")), None);
    assert_eq!(
        token(acct(Provider::JiraCloud, "x.atlassian.net", "")).as_deref(),
        Some("https://id.atlassian.com/manage-profile/security/api-tokens")
    );
    assert_eq!(token(acct(Provider::JiraDataCenter, "jira.acme.io", "")).as_deref(), Some("https://jira.acme.io/secure/ViewProfile.jspa"));

    let az = parse_remote("https://dev.azure.com/contoso/Web/_git/app", &[]).unwrap();
    assert_eq!(
        (az.display(), az.issue_url("4 2")),
        ("contoso/Web/app".into(), "https://dev.azure.com/contoso/Web/_workitems/edit/4%202".into())
    );
    let dc = parse_remote("https://bitbucket.acme.io/scm/PROJ/api.git", &[]).unwrap();
    assert_eq!((dc.display(), dc.issue_url("1")), ("PROJ/api".into(), String::new()));
    let jira = Target::jira(&acct(Provider::JiraCloud, "x.atlassian.net", ""), "");
    assert_eq!((jira.display(), jira.issue_url("AB-1")), ("x.atlassian.net".into(), "https://x.atlassian.net/browse/AB-1".into()));
    assert_eq!(Target::jira(&acct(Provider::JiraDataCenter, "jira.acme.io", ""), "AB").display(), "jira.acme.io · AB");
    assert_eq!(link_line(Provider::Bitbucket, Provider::Bitbucket, "5").as_deref(), Some("Resolves #5"));
    assert_eq!(link_line(Provider::GitLab, Provider::GitLab, "5").as_deref(), Some("Closes #5"));
    assert_eq!(link_line(Provider::BitbucketDataCenter, Provider::BitbucketDataCenter, "5").as_deref(), Some("#5"));
    assert_eq!(link_line(Provider::AzureDevOps, Provider::JiraCloud, "AB-1").as_deref(), Some("AB-1"));
    assert_eq!(link_line(Provider::GitHub, Provider::AzureDevOps, "5").as_deref(), Some("Closes #5"));
}
