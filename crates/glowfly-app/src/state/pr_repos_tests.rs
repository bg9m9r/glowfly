use super::*;
use glowfly_core::forge::parse_remote;
use glowfly_core::repo::Tracking;

fn urls(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect()
}

fn t(url: &str) -> Target {
    parse_remote(url, &[]).unwrap()
}

fn names(repos: &[PrRepo]) -> Vec<&str> {
    repos.iter().map(|r| r.remote.as_str()).collect()
}

fn forge_with(remotes: &[(&str, &str)], host_remote: &str) -> ForgeState {
    let host = t(remotes.iter().find(|(n, _)| *n == host_remote).unwrap().1);
    let mut fs = ForgeState::new(Some(host.clone()), host_remote.into(), None);
    fs.pr_repos = list(&urls(remotes), host_remote, &host, &[]);
    fs
}

fn branch(tracking: Option<(&str, &str)>) -> BranchInfo {
    BranchInfo {
        name: "topic".into(),
        oid: None,
        is_head: false,
        upstream: None,
        ahead: 0,
        behind: 0,
        tracking: tracking.map(|(remote, merge)| Tracking { remote: remote.into(), merge: merge.into() }),
        upstream_ref: None,
        merged: false,
    }
}

#[test]
fn lists_the_remotes_on_the_hosts_account() {
    let folder = glowfly_core::paths::temp_dir().join("app.git").display().to_string();
    let remotes = [
        ("zed", "https://github.com/zed/app.git"),
        ("ada", "git@github.com:ada/app.git"),
        ("origin", "https://github.com/team/app.git"),
        ("lab", "https://gitlab.com/team/app.git"),
        ("disk", folder.as_str()),
        ("mirror", "git@github.com:Team/App"),
    ];
    let host = t("https://github.com/team/app.git");
    let repos = list(&urls(&remotes), "origin", &host, &[]);
    assert_eq!(names(&repos), ["origin", "ada", "zed"], "the host first, others by name, one per repository");
    assert_eq!(repos[0].target, host);
    assert_eq!(label(&repos[1]), "ada/app (ada)");
}

#[test]
fn another_azure_organization_is_not_listed() {
    let remotes = [("origin", "https://dev.azure.com/org/proj/_git/app"), ("other", "https://dev.azure.com/else/proj/_git/app")];
    let fs = forge_with(&remotes, "origin");
    assert_eq!(names(&fs.pr_repos), ["origin"]);
}

#[test]
fn the_base_is_a_remote_named_exactly_upstream() {
    let remotes = [("origin", "https://github.com/ada/app.git"), ("upstream", "https://github.com/team/app.git")];
    assert_eq!(base_remote(&forge_with(&remotes, "origin")), "upstream");
    let remotes = [("origin", "https://github.com/ada/app.git"), ("Upstream", "https://github.com/team/app.git")];
    assert_eq!(base_remote(&forge_with(&remotes, "origin")), "origin");
    // An `upstream` on another service is no PR repository.
    let remotes = [("origin", "https://github.com/ada/app.git"), ("upstream", "https://gitlab.com/team/app.git")];
    assert_eq!(base_remote(&forge_with(&remotes, "origin")), "origin");
}

#[test]
fn a_branch_is_pushed_to_the_pr_repository_it_tracks() {
    let remotes =
        [("origin", "https://github.com/team/app.git"), ("ada", "https://github.com/ada/app.git"), ("lab", "https://gitlab.com/a/b")];
    let fs = forge_with(&remotes, "origin");
    assert_eq!(push_remote(&branch(Some(("ada", "topic"))), &fs), "ada");
    assert_eq!(push_remote(&branch(Some(("lab", "topic"))), &fs), "origin");
    assert_eq!(push_remote(&branch(Some((".", "main"))), &fs), "origin");
    assert_eq!(push_remote(&branch(None), &fs), "origin");
}

#[test]
fn the_target_default_order() {
    let names = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let all = names(&["develop", "main", "release", "trunk"]);
    assert_eq!(first_target(&all, Some("release"), Some("trunk"), Some("develop")), "release");
    assert_eq!(first_target(&all, Some("gone"), Some("trunk"), Some("develop")), "trunk");
    assert_eq!(first_target(&all, None, None, Some("release")), "release");
    assert_eq!(first_target(&all, None, None, Some("gone")), "main");
    assert_eq!(first_target(&names(&["develop", "x"]), None, None, None), "develop");
    assert_eq!(first_target(&names(&["b", "a"]), None, None, None), "b");
    assert_eq!(first_target(&[], None, None, None), "main");
}

#[test]
fn an_empty_pick_means_the_code_hosts_remote() {
    let fs = forge_with(&[("origin", "https://github.com/team/app.git")], "origin");
    let pick = PrRepoPick::default();
    assert_eq!((pick.remote_from(&fs), pick.remote_into(&fs)), ("origin", "origin"));
    let pick = PrRepoPick { branch: "topic".into(), from: "ada".into(), into: "upstream".into() };
    assert_eq!((pick.remote_from(&fs), pick.remote_into(&fs)), ("ada", "upstream"));
}
