use super::*;
use crate::gitcli::run;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use std::path::Path;

fn v(a: &[&str]) -> Vec<String> {
    args(a)
}

fn info(name: &str, urls: &[&str], push: &[&str]) -> RemoteInfo {
    RemoteInfo { name: name.into(), urls: v(urls), push_urls: v(push), skip_fetch_all: false }
}

#[test]
fn remote_name_checks() {
    assert_eq!(check_remote_name("", &[], None), Err(RemoteNameProblem::Empty));
    let cases = [
        ("-x", "Remote names can't start with a dash."),
        ("a b", "Remote names can't contain spaces."),
        ("a\tb", "Remote names can't contain spaces."),
        ("a\0b", "Remote names can't contain control characters."),
        ("a..b", "Remote names can't contain two dots in a row."),
        ("x.lock", "No part of a remote name can end in .lock."),
        ("a/", "Remote names can't start or end with a slash, or have two in a row."),
        ("/a", "Remote names can't start or end with a slash, or have two in a row."),
        ("a//b", "Remote names can't start or end with a slash, or have two in a row."),
        (".a", "Parts of a remote name can't start with a dot, and the name can't end with one."),
        ("a.", "Parts of a remote name can't start with a dot, and the name can't end with one."),
        ("a@{b", "Remote names can't contain @{."),
        ("a:b", "Remote names can't contain ~ ^ : ? * [ or a backslash."),
    ];
    for (name, reason) in cases {
        assert!(!is_valid_remote_name(name), "{name:?}");
        assert_eq!(check_remote_name(name, &[], None), Err(RemoteNameProblem::Invalid(reason)), "{name:?}");
    }
    for name in ["team/fork", "origin", "a.b", "my-remote_2"] {
        assert_eq!(check_remote_name(name, &["other"], None), Ok(()), "{name}");
    }
    assert_eq!(check_remote_name("origin", &["origin"], None), Err(RemoteNameProblem::Exists("origin".into())));
    assert_eq!(check_remote_name("ORIGIN", &["origin"], None), Err(RemoteNameProblem::Exists("origin".into())));
    assert_eq!(RemoteNameProblem::Exists("origin".into()).message(), "A remote named origin already exists.");
    assert_eq!(RemoteNameProblem::Empty.message(), "Enter a remote name.");
    // A case-only rename would leave refs and config half-moved on Windows and macOS.
    assert_eq!(check_remote_name("Origin", &["origin", "b"], Some("origin")), Err(RemoteNameProblem::CaseOnly));
    assert_eq!(check_remote_name("ORIGIN", &["origin", "b"], Some("origin")), Err(RemoteNameProblem::CaseOnly));
    assert_eq!(check_remote_name("Origin2", &["origin", "b"], Some("origin")), Ok(()));
    assert_eq!(check_remote_name("origin", &["origin", "b"], Some("origin")), Ok(()));
    assert_eq!(check_remote_name("B", &["origin", "b"], Some("origin")), Err(RemoteNameProblem::Exists("b".into())));
}

#[test]
fn url_checks() {
    assert_eq!(check_url(""), Err(UrlProblem::Empty));
    assert_eq!(check_url("-oProxyCommand=x"), Err(UrlProblem::Dash));
    assert_eq!(check_url("a\u{7}b"), Err(UrlProblem::Control));
    assert_eq!(check_url("C:/My Repos/x.git"), Ok(()));
    assert_eq!(check_url("git@host:o/r.git"), Ok(()));
    assert_eq!(UrlProblem::Empty.message(false), "Enter a fetch URL.");
    assert_eq!(UrlProblem::Empty.message(true), "");
    assert_eq!(UrlProblem::Dash.message(true), "URLs can't start with a dash.");
    assert_eq!(UrlProblem::Control.message(false), "URLs can't contain control characters.");
}

#[test]
fn exact_regex_escapes() {
    assert_eq!(exact_regex("https://h/x+y(1).git"), r"^https://h/x\+y\(1\)\.git$");
    assert_eq!(exact_regex(r"C:\repos\x"), r"^C:\\repos\\x$");
    assert_eq!(exact_regex("a|b[c]{d}*?^$"), r"^a\|b\[c\]\{d\}\*\?\^\$$");
}

#[test]
fn add_cmds_lines() {
    assert_eq!(add_cmds("up", "https://h/r.git", None, false), vec![v(&["remote", "add", "up", "https://h/r.git"])]);
    assert_eq!(
        add_cmds("up", "u", Some("p"), false),
        vec![v(&["remote", "add", "up", "u"]), v(&["remote", "set-url", "--push", "up", "p"])]
    );
    assert_eq!(
        add_cmds("team/fork", "u", None, true),
        vec![v(&["remote", "add", "team/fork", "u"]), v(&["fetch", "--prune", "team/fork"])]
    );
    assert_eq!(fetch_cmd("o"), v(&["fetch", "--prune", "o"]));
    assert_eq!(remove_cmd("o"), v(&["remote", "remove", "o"]));
}

#[test]
fn edit_cmds_lines() {
    let one = info("origin", &["u1"], &[]);
    let edit = |name, url, push| RemoteEdit { name, url, push_url: push };
    assert!(edit_cmds(&one, &edit("origin", "u1", None)).is_empty());
    assert_eq!(edit_cmds(&one, &edit("up", "u1", None)), vec![v(&["remote", "rename", "origin", "up"])]);
    assert_eq!(edit_cmds(&one, &edit("origin", "u2", None)), vec![v(&["remote", "set-url", "origin", "u2"])]);
    let two = info("origin", &["u.1", "u2"], &[]);
    assert_eq!(edit_cmds(&two, &edit("origin", "u3", None)), vec![v(&["remote", "set-url", "origin", "u3", r"^u\.1$"])]);
    assert_eq!(edit_cmds(&one, &edit("origin", "u1", Some("p"))), vec![v(&["remote", "set-url", "--push", "origin", "p"])]);
    let pushed = info("origin", &["u1"], &["p1"]);
    assert!(edit_cmds(&pushed, &edit("origin", "u1", Some("p1"))).is_empty());
    assert_eq!(edit_cmds(&pushed, &edit("origin", "u1", Some("p2"))), vec![v(&["remote", "set-url", "--push", "origin", "p2"])]);
    let pushed2 = info("origin", &["u1"], &["p+1", "p2"]);
    assert_eq!(
        edit_cmds(&pushed2, &edit("origin", "u1", Some("p3"))),
        vec![v(&["remote", "set-url", "--push", "origin", "p3", r"^p\+1$"])]
    );
    assert_eq!(edit_cmds(&pushed, &edit("origin", "u1", None)), vec![v(&["remote", "set-url", "--delete", "--push", "origin", "^p1$"])]);
    assert_eq!(
        edit_cmds(&pushed, &edit("up", "u2", Some("p2"))),
        vec![
            v(&["remote", "rename", "origin", "up"]),
            v(&["remote", "set-url", "up", "u2"]),
            v(&["remote", "set-url", "--push", "up", "p2"]),
        ]
    );
}

fn run_all(dir: &Path, cmds: &[Vec<String>]) {
    for c in cmds {
        let (ok, out) = run(dir, c);
        assert!(ok, "git {c:?} failed: {out}");
    }
}

fn read(dir: &Path, name: &str) -> RemoteInfo {
    let snap = load(dir, &LoadOptions::all(10)).unwrap();
    snap.remote_info.into_iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no remote {name}"))
}

#[test]
fn edit_cmds_run_with_git() {
    let dir = scratch_repo("remote-ops-edit");
    // (a) An insteadOf rule rewrites the URL git sees; changing the only URL still works.
    git(&dir, &["config", "url.https://example.com/.insteadOf", "ex:"]);
    git(&dir, &["remote", "add", "a", "ex:o/r.git"]);
    let old = read(&dir, "a");
    assert_eq!(old.urls, v(&["ex:o/r.git"]));
    run_all(&dir, &edit_cmds(&old, &RemoteEdit { name: "a", url: "ex:o/other.git", push_url: None }));
    assert_eq!(read(&dir, "a").urls, v(&["ex:o/other.git"]));

    // (b) Two push URLs: changing the first keeps the second.
    git(&dir, &["remote", "add", "b", "https://h/b.git"]);
    git(&dir, &["config", "--add", "remote.b.pushurl", "https://h/p1.git"]);
    git(&dir, &["config", "--add", "remote.b.pushurl", "https://h/p2.git"]);
    let old = read(&dir, "b");
    run_all(&dir, &edit_cmds(&old, &RemoteEdit { name: "b", url: "https://h/b.git", push_url: Some("https://h/new.git") }));
    assert_eq!(read(&dir, "b").push_urls, v(&["https://h/new.git", "https://h/p2.git"]));

    // (c) Clearing a push URL with regex characters removes only that one.
    git(&dir, &["remote", "add", "c", "https://h/c.git"]);
    git(&dir, &["config", "--add", "remote.c.pushurl", "https://h/x+y(1).git"]);
    git(&dir, &["config", "--add", "remote.c.pushurl", "https://h/xxy1.git"]);
    let old = read(&dir, "c");
    let mut first_only = old.clone();
    first_only.push_urls.truncate(1);
    run_all(&dir, &edit_cmds(&first_only, &RemoteEdit { name: "c", url: "https://h/c.git", push_url: None }));
    assert_eq!(read(&dir, "c").push_urls, v(&["https://h/xxy1.git"]));

    // (d) Rename, then set-url under the new name.
    git(&dir, &["remote", "add", "d", "https://h/d.git"]);
    let old = read(&dir, "d");
    run_all(&dir, &edit_cmds(&old, &RemoteEdit { name: "team/d", url: "https://h/d2.git", push_url: Some("https://h/dp.git") }));
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    assert!(!snap.remote_info.iter().any(|r| r.name == "d"));
    assert_eq!(read(&dir, "team/d"), info("team/d", &["https://h/d2.git"], &["https://h/dp.git"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn display_url_hides_password() {
    assert_eq!(display_url("https://u:secret@h/r"), "https://u:***@h/r");
    assert_eq!(display_url("https://u:secret@h:8443/r"), "https://u:***@h:8443/r");
    for same in ["https://u@h/r", "git@h:o/r", "C:/repos/x", "/srv/git/x.git", "https://h:8443/r"] {
        assert_eq!(display_url(same), same);
    }
}
