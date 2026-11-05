//! Marking the journal's rows whose commit nothing keeps any more.

use super::reach::unreachable;
use super::*;
use crate::testutil::*;
use std::path::Path;

fn commit(dir: &Path, text: &str) -> Oid {
    write(dir, "a.txt", text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", text]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, &[("gc.auto", "0")]);
    dir
}

/// Whether every row of commit `oid` is marked (and there is at least one).
fn marked(dir: &Path, oid: Oid) -> bool {
    let tl = load(dir).unwrap();
    let rows: Vec<&Row> = tl.rows.iter().filter(|r| r.oid == oid).collect();
    assert!(!rows.is_empty(), "rows for {oid}");
    rows.iter().all(|r| r.unreachable)
}

#[test]
fn a_reset_away_state_is_marked() {
    let dir = repo("journal-reach-reset");
    let one = commit(&dir, "one");
    let two = commit(&dir, "two");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    let tl = load(&dir).unwrap();
    let of_two: Vec<&Row> = tl.rows.iter().filter(|r| r.oid == two).collect();
    assert!(!of_two.is_empty() && of_two.iter().all(|r| r.unreachable), "{:#?}", tl.rows);
    let current: Vec<&Row> = tl.rows.iter().filter(|r| r.current).collect();
    assert!(!current.is_empty(), "main (with HEAD folded in) is current: {:#?}", tl.rows);
    assert!(current.iter().all(|r| r.oid == one && !r.unreachable));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tag_a_remote_branch_or_another_worktree_keeps_it() {
    let dir = repo("journal-reach-kept");
    commit(&dir, "one");
    let two = commit(&dir, "two");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    assert!(marked(&dir, two));
    let id = two.to_string();

    git(&dir, &["tag", "t", &id]);
    assert!(!marked(&dir, two), "a tag keeps it");
    git(&dir, &["tag", "-d", "t"]);
    assert!(marked(&dir, two));

    git(&dir, &["update-ref", "refs/remotes/origin/x", &id]);
    assert!(!marked(&dir, two), "a remote branch keeps it");
    git(&dir, &["update-ref", "-d", "refs/remotes/origin/x"]);
    assert!(marked(&dir, two));

    let wt = crate::paths::temp_dir().join(format!("glowfly-test-journal-reach-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "--detach", &wt.display().to_string(), &id]);
    assert!(!marked(&dir, two), "another worktree's HEAD keeps it");
    assert!(!marked(&wt, two), "and so does its own, seen from there");
    git(&dir, &["worktree", "remove", "--force", &wt.display().to_string()]);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_branchs_commits_are_marked() {
    let dir = repo("journal-reach-deleted");
    commit(&dir, "one");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    let on_b = commit(&dir, "on b");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["branch", "-q", "-D", "b"]);
    let tl = load(&dir).unwrap();
    let head: Vec<&Row> = tl.rows.iter().filter(|r| r.subject == Subject::Head && r.oid == on_b).collect();
    assert!(!head.is_empty() && head.iter().all(|r| r.unreachable), "{:#?}", tl.rows);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unreachable_ignores_missing_commits() {
    let dir = repo("journal-reach-missing");
    commit(&dir, "one");
    let r = Repository::open(&dir).unwrap();
    let missing = Oid::from_bytes(&[0xff; 20]).unwrap();
    assert!(unreachable(&r, [missing].into_iter()).is_empty());
    drop(r);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_offsite_worktree_s_head_is_read_from_its_admin_dir() {
    let dir = repo("journal-reach-offsite");
    commit(&dir, "one");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let on_side = commit(&dir, "on side");
    git(&dir, &["checkout", "-q", "main"]);
    let admin = dir.join(".git/worktrees/wt");
    std::fs::create_dir_all(&admin).unwrap();
    write(&admin, "HEAD", "ref: refs/heads/side\n");
    write(&admin, "commondir", "../..\n");
    let r = Repository::open(&dir).unwrap();
    assert_eq!(crate::git_file::worktree_head(&r, "wt"), Some(on_side), "a branch it has checked out");
    write(&admin, "HEAD", "ref: refs/../../x\n");
    assert_eq!(crate::git_file::worktree_head(&r, "wt"), None, "a ref outside refs/ isn't read");
    git(&dir, &["branch", "-q", "-D", "side"]);
    assert!(marked(&dir, on_side));
    // Now only the worktree's detached HEAD keeps it, and its folder (written after the last
    // git command) is never opened.
    write(&admin, "HEAD", &format!("{on_side}\n"));
    let far = net_place().join("wt/.git").display().to_string().replace('\\', "/");
    write(&admin, "gitdir", &format!("{far}\n"));
    assert!(unreachable(&r, [on_side].into_iter()).is_empty(), "kept by the offsite worktree's HEAD");
    drop(r);
    let _ = std::fs::remove_dir_all(&dir);
}
