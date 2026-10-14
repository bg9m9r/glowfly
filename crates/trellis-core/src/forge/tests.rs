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
    let auth = resolve_auth(&account, &std::env::temp_dir(), false, true).expect("no credentials");
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
