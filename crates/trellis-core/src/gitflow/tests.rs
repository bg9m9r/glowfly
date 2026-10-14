use super::*;
use crate::gitcli::run;
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, scratch_repo, write};

fn defaults() -> GitFlowSettings {
    GitFlowSettings {
        production: "main".into(),
        development: "develop".into(),
        feature: DEFAULT_FEATURE.into(),
        release: DEFAULT_RELEASE.into(),
        hotfix: DEFAULT_HOTFIX.into(),
        version_tag: String::new(),
    }
}

fn branches<'a>(locals: &[&'a str], remote: &[(&'a str, &'a str)]) -> Branches<'a> {
    Branches { locals: locals.to_vec(), remote: remote.to_vec(), tags: vec![], head: None, has_commits: true }
}

fn initialized() -> GitFlowConfig {
    GitFlowConfig {
        production: Some("main".into()),
        development: Some("develop".into()),
        feature: Some("feature/".into()),
        release: Some("release/".into()),
        hotfix: Some("hotfix/".into()),
        version_tag: Some(String::new()),
        bugfix: None,
        support: None,
    }
}

fn cmd(a: &[&str]) -> Vec<String> {
    args(a)
}

fn config_cmds(bugfix: bool, support: bool) -> Vec<Vec<String>> {
    let mut out = vec![
        cmd(&["config", "--local", "gitflow.branch.master", "main"]),
        cmd(&["config", "--local", "gitflow.branch.develop", "develop"]),
        cmd(&["config", "--local", "gitflow.prefix.feature", "feature/"]),
    ];
    if bugfix {
        out.push(cmd(&["config", "--local", "gitflow.prefix.bugfix", "bugfix/"]));
    }
    out.push(cmd(&["config", "--local", "gitflow.prefix.release", "release/"]));
    out.push(cmd(&["config", "--local", "gitflow.prefix.hotfix", "hotfix/"]));
    if support {
        out.push(cmd(&["config", "--local", "gitflow.prefix.support", "support/"]));
    }
    out.push(cmd(&["config", "--local", "gitflow.prefix.versiontag", ""]));
    out
}

fn committed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

#[test]
fn read_config_and_load() {
    let dir = committed("gitflow-read");
    let repo = git2::Repository::open(&dir).unwrap();
    assert_eq!(read_config(&repo), GitFlowConfig::default());
    git(&dir, &["config", "gitflow.branch.master", "main"]);
    git(&dir, &["config", "gitflow.branch.develop", "develop"]);
    git(&dir, &["config", "gitflow.prefix.feature", "feature/"]);
    git(&dir, &["config", "gitflow.prefix.release", "release/"]);
    git(&dir, &["config", "gitflow.prefix.hotfix", "hotfix/"]);
    git(&dir, &["config", "gitflow.prefix.versiontag", ""]);
    let cfg = read_config(&repo);
    assert_eq!(cfg, initialized());
    assert!(cfg.is_initialized());
    let snap = load(&dir, &LoadOptions::all(50)).unwrap();
    assert_eq!(snap.gitflow, cfg);

    // A key with no value at all reads as "".
    let path = dir.join(".git/config");
    let text = std::fs::read_to_string(&path).unwrap().replace("versiontag = ", "versiontag");
    std::fs::write(&path, text).unwrap();
    let repo = git2::Repository::open(&dir).unwrap();
    assert_eq!(read_config(&repo).version_tag.as_deref(), Some(""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_initialized() {
    assert!(initialized().is_initialized());
    let without = |f: fn(&mut GitFlowConfig)| {
        let mut c = initialized();
        f(&mut c);
        c.is_initialized()
    };
    assert!(!without(|c| c.development = None));
    assert!(!without(|c| c.development = Some(String::new())));
    assert!(!without(|c| c.development = Some("main".into())));
    assert!(!without(|c| c.development = Some("MAIN".into())));
    assert!(!without(|c| c.release = None));
    assert!(without(|c| c.version_tag = None));
    let mut c = initialized();
    c.version_tag = None;
    assert_eq!(c.settings().unwrap().version_tag, "");
    assert_eq!(c.settings().unwrap().production, "main");
    assert_eq!(GitFlowConfig::default().settings(), None);
}

#[test]
fn suggested_values() {
    let cfg = GitFlowConfig { production: Some("trunk".into()), feature: Some("feat/".into()), ..Default::default() };
    let s = suggested(&cfg, &branches(&["main"], &[]));
    assert_eq!((s.production.as_str(), s.feature.as_str()), ("trunk", "feat/"));
    let none = GitFlowConfig::default();
    let s = suggested(&none, &branches(&["master", "main"], &[]));
    assert_eq!(s, defaults());
    assert_eq!(suggested(&none, &branches(&["master", "x"], &[])).production, "master");
    assert_eq!(suggested(&none, &branches(&["x"], &[("origin", "main")])).production, "main");
    assert_eq!(suggested(&none, &branches(&["x"], &[("origin", "master")])).production, "master");
    let mut b = branches(&["x", "trunk"], &[]);
    b.head = Some("trunk");
    assert_eq!(suggested(&none, &b).production, "trunk");
    assert_eq!(suggested(&none, &branches(&[], &[])).production, "main");
    assert_eq!(s.development, "develop");
    assert_eq!((s.release.as_str(), s.hotfix.as_str(), s.version_tag.as_str()), ("release/", "hotfix/", ""));
}

#[test]
fn settings_checks() {
    use SettingsProblem as P;
    let check = |f: fn(&mut GitFlowSettings)| {
        let mut s = defaults();
        f(&mut s);
        check_settings(&s)
    };
    assert_eq!(check(|_| {}), Ok(()));
    assert_eq!(check(|s| s.version_tag = "v".into()), Ok(()));
    assert_eq!(check(|s| s.production.clear()), Err(P::EmptyBranch { development: false }));
    assert_eq!(check(|s| s.development.clear()), Err(P::EmptyBranch { development: true }));
    assert_eq!(check(|s| s.development = "dev elop".into()), Err(P::InvalidBranch("dev elop".into())));
    assert_eq!(check(|s| s.development = "Main".into()), Err(P::SameBranches));
    assert_eq!(check(|s| s.development = "main/x".into()), Err(P::BranchesClash));
    assert_eq!(check(|s| s.release.clear()), Err(P::EmptyPrefix(FlowKind::Release)));
    assert_eq!(check(|s| s.hotfix = "hot..fix/".into()), Err(P::InvalidPrefix(FlowKind::Hotfix)));
    assert_eq!(check(|s| s.release = "feature/".into()), Err(P::SamePrefixes(FlowKind::Feature, FlowKind::Release)));
    assert_eq!(check(|s| s.hotfix = "feature/x/".into()), Err(P::SamePrefixes(FlowKind::Feature, FlowKind::Hotfix)));
    assert_eq!(
        check(|s| s.development = "feature/dev".into()),
        Err(P::BranchHasPrefix { branch: "feature/dev".into(), prefix: "feature/".into() })
    );
    assert_eq!(check(|s| s.version_tag = "v..".into()), Err(P::InvalidTagPrefix));
    assert!(P::SamePrefixes(FlowKind::Feature, FlowKind::Release).message().contains("feature and release"));
}

#[test]
fn init_commands() {
    let s = defaults();
    let cfg = GitFlowConfig::default();
    assert_eq!(init_cmds(&s, &cfg, &branches(&["main", "develop"], &[])), Ok(config_cmds(true, true)));
    let configured = GitFlowConfig { bugfix: Some("bug/".into()), support: Some("sup/".into()), ..initialized() };
    assert_eq!(init_cmds(&s, &configured, &branches(&["main", "develop"], &[])), Ok(config_cmds(false, false)));

    let cmds = init_cmds(&s, &cfg, &branches(&["main"], &[])).unwrap();
    assert_eq!(cmds[0], cmd(&["branch", "--no-track", "develop", "refs/heads/main"]));
    assert_eq!(cmds[1..], config_cmds(true, true)[..]);
    let cmds = init_cmds(&s, &cfg, &branches(&["main"], &[("upstream", "develop"), ("origin", "develop")])).unwrap();
    assert_eq!(cmds[0], cmd(&["branch", "--track", "develop", "upstream/develop"]));
    let mut b = branches(&["main"], &[]);
    b.remote = vec![("origin", "develop")];
    assert_eq!(init_cmds(&s, &cfg, &b).unwrap()[0], cmd(&["branch", "--track", "develop", "origin/develop"]));
    let cmds = init_cmds(&s, &cfg, &branches(&["develop"], &[("origin", "main")])).unwrap();
    assert_eq!(cmds[0], cmd(&["branch", "--track", "main", "origin/main"]));
    assert_eq!(cmds[1], cmd(&["config", "--local", "gitflow.branch.master", "main"]));
    let cmds = init_cmds(&s, &cfg, &branches(&["other"], &[("origin", "main")])).unwrap();
    assert_eq!(cmds[1], cmd(&["branch", "--no-track", "develop", "refs/heads/main"]));

    assert_eq!(init_cmds(&s, &cfg, &branches(&["other"], &[])), Err(InitProblem::MissingProduction("main".into())));
    let mut empty = branches(&[], &[]);
    empty.has_commits = false;
    assert_eq!(init_cmds(&s, &cfg, &empty), Err(InitProblem::NoCommits));
    assert_eq!(
        init_cmds(&s, &cfg, &branches(&["main", "develop/x"], &[])),
        Err(InitProblem::DevelopmentName(NameProblem::Clashes("develop/x".into()), "develop".into()))
    );
}

#[test]
fn init_end_to_end() {
    let dir = committed("gitflow-init");
    let snap = load(&dir, &LoadOptions::all(50)).unwrap();
    let b = Branches::of(&snap);
    let s = suggested(&snap.gitflow, &b);
    for c in init_cmds(&s, &snap.gitflow, &b).unwrap() {
        let (ok, out) = run(&dir, &c);
        assert!(ok, "{c:?}: {out}");
    }
    let rev = |r: &str| run(&dir, &args(&["rev-parse", r])).1;
    assert_eq!(rev("develop"), rev("main"));
    assert_eq!(run(&dir, &args(&["symbolic-ref", "--short", "HEAD"])).1.trim(), "main");
    let (ok, out) = run(&dir, &args(&["config", "--get", "gitflow.prefix.versiontag"]));
    assert!(ok);
    assert_eq!(out.trim(), "");
    let repo = git2::Repository::open(&dir).unwrap();
    assert!(read_config(&repo).is_initialized());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn start_checks() {
    let s = defaults();
    let b = branches(&["main", "develop", "feature/Login"], &[]);
    let name = |p: NameProblem, full: &str| Err(StartProblem::Name(p, full.into()));
    assert_eq!(check_start(&s, FlowKind::Feature, "  ", &b), name(NameProblem::Empty, "feature/"));
    assert_eq!(check_start(&s, FlowKind::Feature, "bad name", &b), name(NameProblem::Invalid, "feature/bad name"));
    assert_eq!(check_start(&s, FlowKind::Feature, "login", &b), name(NameProblem::Exists("feature/Login".into()), "feature/login"));
    let b2 = branches(&["main", "develop", "feature/login/x"], &[]);
    assert_eq!(check_start(&s, FlowKind::Feature, "login", &b2), name(NameProblem::Clashes("feature/login/x".into()), "feature/login"));
    let no_dev = branches(&["main"], &[("origin", "develop")]);
    assert_eq!(
        check_start(&s, FlowKind::Feature, "login", &no_dev),
        Err(StartProblem::MissingBase { kind: FlowKind::Feature, base: "develop".into() })
    );
    assert_eq!(check_start(&s, FlowKind::Hotfix, "1.0.1", &no_dev), Ok("hotfix/1.0.1".into()));
    let no_main = branches(&["develop"], &[]);
    let missing = check_start(&s, FlowKind::Hotfix, "1.0.1", &no_main).unwrap_err();
    assert_eq!(missing, StartProblem::MissingBase { kind: FlowKind::Hotfix, base: "main".into() });
    assert!(missing.message().contains("hotfix"));

    let v = GitFlowSettings { version_tag: "v".into(), ..defaults() };
    let mut tagged = branches(&["main", "develop"], &[]);
    tagged.tags = vec!["v1.2.0"];
    assert_eq!(check_start(&v, FlowKind::Release, "1.2.0", &tagged), Err(StartProblem::TagExists("v1.2.0".into())));
    assert_eq!(check_start(&v, FlowKind::Feature, "1.2.0", &tagged), Ok("feature/1.2.0".into()));
    assert_eq!(check_start(&s, FlowKind::Release, "1.2.0", &tagged), Ok("release/1.2.0".into()));

    let ok = check_start(&s, FlowKind::Feature, " login ", &branches(&["main", "develop"], &[])).unwrap();
    assert_eq!(ok, "feature/login");
    assert_eq!(start_cmd(&s, FlowKind::Feature, &ok), cmd(&["checkout", "-b", "feature/login", "refs/heads/develop"]));
    assert_eq!(start_cmd(&s, FlowKind::Hotfix, "hotfix/1.0.1")[3], "refs/heads/main");
    assert_eq!(s.base(FlowKind::Release), "develop");
}

#[test]
fn version_suggestions() {
    let tags = ["v1.2.0", "v1.10.0", "v0.9", "other", "v2.0.0-rc1"];
    assert_eq!(suggest_version(&tags, "v", FlowKind::Release).as_deref(), Some("1.11.0"));
    assert_eq!(suggest_version(&tags, "v", FlowKind::Hotfix).as_deref(), Some("1.10.1"));
    assert_eq!(suggest_version(&["1.0"], "", FlowKind::Release).as_deref(), Some("1.1.0"));
    assert_eq!(suggest_version(&["1.0"], "", FlowKind::Hotfix).as_deref(), Some("1.0.1"));
    assert_eq!(suggest_version(&["other", "1", "1.2.3.4", "a.b"], "", FlowKind::Release), None);
    assert_eq!(suggest_version(&["1.0"], "v", FlowKind::Release), None);
    assert_eq!(suggest_version(&tags, "v", FlowKind::Feature), None);
}

#[test]
fn open_branch_list() {
    let s = defaults();
    let b = branches(&["main", "develop", "release/1.2.0", "feature/x"], &[]);
    assert_eq!(open_branches(&s, FlowKind::Release, &b), ["release/1.2.0"]);
    assert!(open_branches(&s, FlowKind::Hotfix, &b).is_empty());
}
