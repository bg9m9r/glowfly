use super::*;
use crate::gitcli::{args, run};
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

const URL: &str = "https://example.com/team/app.git";

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn shallow(limit: HistoryLimit, branch: Option<&str>) -> CloneOptions {
    CloneOptions { shallow: Some(Shallow { limit, branch: branch.map(str::to_owned) }) }
}

#[test]
fn full_clone_cmd() {
    assert_eq!(clone_cmd(URL, "app", &CloneOptions::default()), ["clone", "--", URL, "app"]);
}

#[test]
fn shallow_clone_cmds() {
    assert_eq!(clone_cmd(URL, "app", &shallow(HistoryLimit::Depth(1), None)), ["clone", "--depth=1", "--single-branch", "--", URL, "app"]);
    assert_eq!(
        clone_cmd(URL, "app", &shallow(HistoryLimit::Depth(50), Some("release/2.0"))),
        ["clone", "--depth=50", "--single-branch", "--branch=release/2.0", "--", URL, "app"]
    );
    assert_eq!(
        clone_cmd(URL, "app", &shallow(HistoryLimit::Since(date(2026, 1, 5)), None)),
        ["clone", "--shallow-since=2026-01-05T00:00:00", "--single-branch", "--", URL, "app"]
    );
}

#[test]
fn local_sources_get_no_local() {
    for src in ["C:\\src\\repo", "/srv/repo", "../repo", "repo"] {
        let cmd = clone_cmd(src, "repo", &shallow(HistoryLimit::Depth(1), None));
        let dashes = cmd.iter().position(|a| a == "--").unwrap();
        assert_eq!(cmd[dashes - 1], "--no-local", "{src}: {cmd:?}");
        assert!(!clone_cmd(src, "repo", &CloneOptions::default()).contains(&"--no-local".to_owned()), "{src}");
    }
}

#[test]
fn local_source_table() {
    let cases = [
        ("https://h/x", false),
        ("ssh://h/x", false),
        ("file:///c/x", false),
        ("git@host:team/app", false),
        ("host:path", false),
        ("C:\\x", true),
        ("C:/x", true),
        ("/srv/x", true),
        ("./x", true),
        ("~/x", true),
        ("\\\\server\\share", true),
        ("repo", true),
    ];
    for (url, local) in cases {
        assert_eq!(is_local_source(url), local, "{url}");
    }
}

#[test]
fn repo_names() {
    assert_eq!(repo_name("https://github.com/o/repo.git"), "repo");
    assert_eq!(repo_name("git@host:team/app"), "app");
    assert_eq!(repo_name("https://example.com/team/app/"), "app");
    assert_eq!(repo_name("C:\\src\\proj"), "proj");
    assert_eq!(repo_name(""), "");
    // Names Windows can't have as a folder.
    assert_eq!(repo_name("https://example.com/o/con.git"), "con-repo");
    assert_eq!(repo_name("https://example.com/o/AUX"), "AUX-repo");
    assert_eq!(repo_name("https://example.com/o/lpt1.d.git"), "lpt1-repo.d");
    assert_eq!(repo_name("https://example.com/o/odd..git"), "odd");
    assert_eq!(repo_name("https://example.com/o/a%7Cb?x=1"), "a%7Cb-x=1");
}

#[test]
fn depth_rules() {
    assert_eq!(parse_depth("1"), Ok(1));
    assert_eq!(parse_depth(" 50 "), Ok(50));
    assert_eq!(parse_depth(""), Err(DepthProblem::Empty));
    for t in ["abc", "-1", "+5", "1.5", " 1 2"] {
        assert_eq!(parse_depth(t), Err(DepthProblem::NotANumber), "{t}");
    }
    assert_eq!(parse_depth("0"), Err(DepthProblem::Zero));
    assert_eq!(parse_depth("000"), Err(DepthProblem::Zero));
    assert_eq!(parse_depth("2147483647"), Ok(2_147_483_647));
    assert_eq!(parse_depth("2147483648"), Err(DepthProblem::TooLarge));
    assert_eq!(parse_depth("99999999999999999999"), Err(DepthProblem::TooLarge));
}

#[test]
fn since_rules() {
    let today = date(2026, 2, 1);
    assert_eq!(parse_since("2026-01-31", today), Ok(date(2026, 1, 31)));
    assert_eq!(parse_since("2026-02-01", today), Ok(today));
    assert_eq!(parse_since(" 1970-01-01 ", today), Ok(date(1970, 1, 1)));
    assert_eq!(parse_since("2026-02-02", today), Err(SinceProblem::Future));
    assert_eq!(parse_since("", today), Err(SinceProblem::Empty));
    for t in ["2026-1-5", "2026/01/05", "20260105", "2026-02-30", "abcd-ef-gh"] {
        assert_eq!(parse_since(t, today), Err(SinceProblem::Invalid), "{t}");
    }
    assert_eq!(parse_since("1969-12-31", today), Err(SinceProblem::BeforeEpoch));
}

#[test]
fn branch_rules() {
    assert_eq!(check_clone_branch(""), Ok(None));
    assert_eq!(check_clone_branch("  "), Ok(None));
    for b in ["main", "release/2.0", "v1.2.3"] {
        assert_eq!(check_clone_branch(b), Ok(Some(b.to_owned())));
    }
    assert_eq!(check_clone_branch(" main "), Ok(Some("main".to_owned())));
    for b in ["-x", "a b", "a..b", "x.lock"] {
        assert_eq!(check_clone_branch(b), Err(()), "{b}");
    }
}

#[test]
fn local_dates() {
    assert_eq!(local_date(1_769_947_200, 0), date(2026, 2, 1));
    // 2026-02-01T00:00Z, an hour west of UTC, is still January 31st.
    assert_eq!(local_date(1_769_904_000, -3600), date(2026, 1, 31));
    assert_eq!(local_date(1_769_904_000, 0), date(2026, 2, 1));
    assert_eq!(local_date(i64::MAX, 3600), date(1970, 1, 1));
}

/// Three commits on `main` and a `dev` branch with one more.
fn source(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    for (i, text) in ["one", "two", "three"].iter().enumerate() {
        write(&dir, "a.txt", text);
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", &format!("commit {i}")]);
    }
    git(&dir, &["checkout", "-q", "-b", "dev"]);
    write(&dir, "b.txt", "dev");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "on dev"]);
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn out(dir: &Path, a: &[&str]) -> String {
    let (ok, o) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {o}");
    o.trim().to_owned()
}

#[test]
fn shallow_clone_of_one_branch_from_a_folder() {
    let src = source("clone-depth");
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let cmd = clone_cmd(&src.display().to_string(), "shallow", &shallow(HistoryLimit::Depth(1), Some("dev")));
    let (ok, o) = run(&parent, &cmd);
    assert!(ok, "{o}");
    let dest = parent.join("shallow");
    assert_eq!(out(&dest, &["rev-parse", "--is-shallow-repository"]), "true");
    assert_eq!(out(&dest, &["rev-list", "--count", "HEAD"]), "1");
    let branches: Vec<String> = out(&dest, &["branch", "-a", "--format=%(refname:short)"]).lines().map(str::to_owned).collect();
    assert_eq!(branches, ["dev", "origin/dev"]);
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&src);
}

#[test]
fn shallow_since_today_keeps_todays_commits() {
    let src = source("clone-since");
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let today = chrono::Local::now().date_naive();
    let cmd = clone_cmd(&src.display().to_string(), "since", &shallow(HistoryLimit::Since(today), None));
    let (ok, o) = run(&parent, &cmd);
    assert!(ok, "{o}");
    assert_eq!(out(&parent.join("since"), &["rev-list", "--count", "HEAD"]), "3");
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&src);
}
