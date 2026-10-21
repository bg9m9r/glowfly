use super::*;
use crate::forge::mock::{Mock, Reply};
use crate::forge::{Account, PrFacets, Provider};
use crate::testutil::{git, scratch_repo, set_config};
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

fn urls(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect()
}

fn gh(owner: &str, repo: &str) -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: owner.into(), project: String::new(), repo: repo.into() }
}

#[test]
fn the_code_host_is_origin_else_the_first_recognized_remote() {
    let both = urls(&[("upstream", "https://github.com/up/widgets.git"), ("origin", "git@github.com:acme/widgets.git")]);
    assert_eq!(code_host(&both, &[]), Some(("origin".into(), gh("acme", "widgets"))));
    let no_origin = urls(&[("mirror", "/srv/git/widgets"), ("fork", "https://github.com/ada/widgets"), ("up", "https://github.com/up/w")]);
    assert_eq!(code_host(&no_origin, &[]), Some(("fork".into(), gh("ada", "widgets"))));
    // An unrecognized origin doesn't count.
    let odd_origin = urls(&[("origin", "https://code.acme.io/team/api.git"), ("gh", "https://github.com/acme/api")]);
    assert_eq!(code_host(&odd_origin, &[]).map(|(n, _)| n), Some("gh".into()));
    assert_eq!(code_host(&urls(&[("origin", "/srv/git/x")]), &[]), None);
    // A self-hosted server under its own name is recognized only once connected.
    let own = urls(&[("origin", "https://code.acme.io/team/api.git")]);
    assert_eq!(code_host(&own, &[]), None);
    let account = Account { provider: Provider::GitHubEnterprise, host: "code.acme.io".into(), org: String::new() };
    let conn = Connection { account, user: "ada".into(), method: "access token".into() };
    let (_, t) = code_host(&own, &[conn]).expect("connected");
    assert_eq!((t.provider, t.owner.as_str(), t.repo.as_str()), (Provider::GitHubEnterprise, "team", "api"));
}

#[test]
fn recognized_lists_each_target_once_origin_first() {
    let list = urls(&[
        ("backup", "https://gitlab.com/g/widgets.git"),
        ("ssh", "git@github.com:acme/widgets.git"),
        ("origin", "https://github.com/acme/widgets.git"),
        ("local", "../widgets.git"),
        ("upstream", "https://github.com/up/widgets"),
    ]);
    let found = recognized(&list, &[]);
    let names: Vec<&str> = found.iter().map(|r| r.remote.as_str()).collect();
    assert_eq!(names, ["origin", "backup", "upstream"], "the ssh remote is the same repository as origin");
    assert_eq!(found[0].target, gh("acme", "widgets"));
    assert_eq!(found[0].url, "https://github.com/acme/widgets.git");
    assert_eq!(found[1].target.provider, Provider::GitLab);
}

#[test]
fn read_repo_finds_the_remotes_and_the_code_host() {
    let dir = scratch_repo("dash-read");
    set_config(
        &dir,
        &[("remote.origin.url", "https://github.com/acme/widgets.git"), ("remote.upstream.url", "git@github.com:up/widgets.git")],
    );
    let r = read_repo(&dir, &[]).unwrap();
    assert!(paths::same_spelling(&r.real_workdir, &paths::resolve(&dir)), "{:?}", r.real_workdir);
    assert_eq!(r.name, dir.file_name().unwrap().to_string_lossy());
    assert!(r.folder.is_some());
    let targets: Vec<(&str, Target)> = r.remotes.iter().map(|t| (t.remote.as_str(), t.target.clone())).collect();
    assert_eq!(targets, [("origin", gh("acme", "widgets")), ("upstream", gh("up", "widgets"))]);
    assert_eq!(r.host, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_repo_applies_instead_of() {
    let dir = scratch_repo("dash-insteadof");
    set_config(&dir, &[("remote.origin.url", "gh:acme/widgets"), ("url.https://github.com/.insteadOf", "gh:")]);
    let r = read_repo(&dir, &[]).unwrap();
    assert_eq!(r.remotes.len(), 1);
    assert_eq!(r.remotes[0].url, "https://github.com/acme/widgets");
    assert_eq!(r.remotes[0].target, gh("acme", "widgets"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_repo_refuses_missing_plain_and_bare_folders() {
    let base = paths::temp_dir().join(format!("trellis-test-dash-folders-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let plain = base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let missing = base.join("missing");
    assert!(matches!(read_repo(&missing, &[]), Err(Error::NotARepository(_))));
    let e = read_repo(&plain, &[]).unwrap_err();
    assert!(matches!(e, Error::NotARepository(_)));
    assert_eq!(unreadable_reason(&e), "not a Git repository");
    let bare = base.join("bare.git");
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare"]);
    assert!(matches!(read_repo(&bare, &[]), Err(Error::Unsupported(_))));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn run_bounded_keeps_order_and_the_limit() {
    let (running, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let out = run_bounded((0..12).collect(), 3, |i: u32| {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(5));
        running.fetch_sub(1, Ordering::SeqCst);
        i * 10
    });
    assert_eq!(out, (0..12).map(|i| i * 10).collect::<Vec<_>>());
    assert!(most.load(Ordering::SeqCst) <= 3, "{}", most.load(Ordering::SeqCst));
    // Limit 0 runs one at a time.
    most.store(0, Ordering::SeqCst);
    let out = run_bounded(vec!["a", "b", "c"], 0, |s| {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
        running.fetch_sub(1, Ordering::SeqCst);
        s.to_uppercase()
    });
    assert_eq!(out, ["A", "B", "C"]);
    assert_eq!(most.load(Ordering::SeqCst), 1);
    assert!(run_bounded(Vec::<u8>::new(), 4, |_| -> u8 { panic!("nothing to run") }).is_empty());
}

fn pr(id: u64, author: &str, awaiting: &[&str], reviewers: &[&str]) -> PullRequest {
    PullRequest {
        id,
        title: format!("PR {id}"),
        description: String::new(),
        source: format!("b{id}"),
        target: "main".into(),
        author: author.into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: PrFacets {
            author_id: author.into(),
            awaiting_ids: awaiting.iter().map(|s| s.to_string()).collect(),
            reviewer_ids: reviewers.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    }
}

#[test]
fn mine_and_review_split_by_author_and_awaited_reviewer() {
    let prs = vec![
        pr(1, "Ada", &[], &[]),
        pr(2, "grace", &["ADA"], &["ada"]),
        pr(3, "linus", &[], &["ada"]),
        pr(4, "ada", &["grace"], &["grace"]),
    ];
    let me = Me { id: "ada".into(), name: "Ada L".into() };
    let (mine, review) = mine_and_review(&prs, &me);
    assert_eq!(mine.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 4]);
    assert_eq!(review.iter().map(|p| p.id).collect::<Vec<_>>(), [2], "#3 was reviewed already");
}

#[test]
fn fetch_target_asks_merge_requests_and_assigned_issues() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects/g%2Fapp/merge_requests?state=opened&per_page=100" => Reply::json("[]"),
        route if route.starts_with("GET gitlab.com/api/v4/projects/g%2Fapp/issues?") => Reply::json("[]"),
        _ => Reply::not_found(),
    });
    let t = parse_remote("https://gitlab.com/g/app.git", &[]).unwrap();
    let auth = Auth::from_token(Provider::GitLab, None, "tok", "test");
    let f = fetch_target(&t, &auth, true, true);
    assert!(f.prs.is_some_and(|r| r.is_ok_and(|v| v.is_empty())));
    assert!(f.issues.is_some_and(|r| r.is_ok_and(|v| v.is_empty())));
    let reqs = m.requests();
    assert_eq!(reqs.len(), 2, "{:?}", reqs.iter().map(|r| r.route()).collect::<Vec<_>>());
    assert!(reqs.iter().all(|r| r.header("Authorization") == Some("Bearer tok")));
    assert!(reqs[1].path.contains("scope=assigned_to_me"), "{}", reqs[1].path);

    let f = fetch_target(&t, &auth, true, false);
    assert!(f.issues.is_none());
    assert_eq!(m.requests().len(), 3, "no issue request");
    drop(m);

    let _m = Mock::start(|_| Reply::status(401, r#"{"message":"401 Unauthorized"}"#));
    let f = fetch_target(&t, &auth, true, true);
    assert!(f.prs.is_some_and(|r| r.is_err_and(|e| e.is_auth())));
    assert!(f.issues.is_some_and(|r| r.is_err_and(|e| e.is_auth())));
}
