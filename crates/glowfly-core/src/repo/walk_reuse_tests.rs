use super::load_more_tests::{branchy, point, raw_commit, same_history};
use super::*;
use crate::testutil::*;
use git2::Oid;
use std::path::Path;

fn reload(dir: &Path, opts: &LoadOptions, before: &Snapshot) -> Snapshot {
    load_reusing(dir, opts, &GraphCache::default(), Some(&before.prior())).unwrap()
}

/// `load_reusing` on top of `before` gives what a fresh load gives, at a count past the history
/// and at one that cuts it short; returns the reusing load of the whole history.
#[track_caller]
fn matches_fresh(dir: &Path, before: &Snapshot, opts: &LoadOptions, what: &str) -> Snapshot {
    let short = LoadOptions { max_commits: 50, ..opts.clone() };
    same_history(&reload(dir, &short, before), &load(dir, &short).unwrap(), &format!("{what}, cut short"));
    let s = reload(dir, opts, before);
    same_history(&s, &load(dir, opts).unwrap(), what);
    s
}

fn head_of(repo: &git2::Repository, name: &str) -> Oid {
    repo.find_reference(name).unwrap().target().unwrap()
}

#[test]
fn a_reload_reusing_commits_matches_a_fresh_load() {
    let (dir, main) = branchy("reuse-reload");
    let repo = git2::Repository::open(&dir).unwrap();
    let opts = LoadOptions::all(1000);
    let t = 1_700_000_000;
    let s = load(&dir, &opts).unwrap();

    let tip = head_of(&repo, "refs/heads/main");
    let one = raw_commit(&repo, &[tip], t, "new one");
    let two = raw_commit(&repo, &[one], t + 10, "new two");
    point(&repo, "refs/heads/main", two);
    let s = matches_fresh(&dir, &s, &opts, "commits added on main");

    let side = raw_commit(&repo, &[main[30]], t + 20, "topic work");
    let merge = raw_commit(&repo, &[two, side], t + 30, "Merge branch 'topic'");
    point(&repo, "refs/heads/topic", side);
    point(&repo, "refs/heads/main", merge);
    let s = matches_fresh(&dir, &s, &opts, "a branch merged");

    repo.set_head("refs/heads/feature").unwrap();
    let s = matches_fresh(&dir, &s, &opts, "another branch checked out");

    point(&repo, "refs/heads/main", one);
    let s = matches_fresh(&dir, &s, &opts, "main reset back two");

    let hidden = LoadOptions { hidden: vec!["refs/heads/docs".into(), "refs/heads/topic".into()], ..opts.clone() };
    let s = matches_fresh(&dir, &s, &hidden, "branches hidden");
    matches_fresh(&dir, &s, &opts, "shown again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reload_after_a_fetch_matches_a_fresh_load() {
    let (work, bare) = with_origin("reuse-fetch");
    for i in 0..5 {
        write(&work, "a.txt", &format!("{i}\n"));
        git(&work, &["commit", "-q", "-am", &format!("local {i}")]);
    }
    git(&work, &["push", "-q", "origin", "main"]);
    let opts = LoadOptions::all(1000);
    let s = load(&work, &opts).unwrap();
    remote_commit(&bare, "main", "b.txt", "from elsewhere");
    remote_commit(&bare, "main", "c.txt", "and more");
    git(&work, &["fetch", "-q", "origin"]);
    let s = matches_fresh(&work, &s, &opts, "fetched");
    assert_eq!(&*s.commits[0].summary, "and more");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn reused_commits_are_not_read_again() {
    let (dir, _) = branchy("reuse-shared");
    let repo = git2::Repository::open(&dir).unwrap();
    let opts = LoadOptions::all(1000);
    let s1 = load(&dir, &opts).unwrap();
    let tip = head_of(&repo, "refs/heads/main");
    point(&repo, "refs/heads/main", raw_commit(&repo, &[tip], 1_700_000_000, "by the same person"));

    let s2 = reload(&dir, &opts, &s1);
    let fresh = load(&dir, &opts).unwrap();
    let author_in = |s: &Snapshot, oid: Oid| s.commits[s.row_of[&oid]].author.clone();
    for c in s1.commits.iter() {
        assert!(author_in(&s2, c.oid).ptr_eq(&c.author), "{} was taken from the earlier load", c.oid);
        assert!(!author_in(&fresh, c.oid).ptr_eq(&c.author), "a fresh load reads it again");
    }
    assert!(s2.commits[0].author.ptr_eq(&s1.commits[0].author), "the new commit's author is the reused one");
    assert_eq!(s2.commits.len(), s1.commits.len() + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deepened_shallow_clone_reads_its_old_roots_again() {
    let src = scratch_repo("reuse-shallow");
    for i in 0..6 {
        write(&src, "a.txt", &format!("{i}\n"));
        git(&src, &["add", "."]);
        git(&src, &["commit", "-q", "-m", &format!("c{i}")]);
    }
    let clone = src.with_file_name(format!("{}-sh", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&clone);
    git(src.parent().unwrap(), &["clone", "-q", "--depth", "2", &file_url(&src), &clone.display().to_string()]);
    let opts = LoadOptions::all(100);
    let s1 = load(&clone, &opts).unwrap();
    assert_eq!(s1.commits.len(), 2);
    let root = s1.commits[1].oid;
    assert!(s1.commits[1].parents.is_empty(), "the shallow root has no parents");

    git(&clone, &["fetch", "-q", "--deepen", "2"]);
    let s2 = reload(&clone, &opts, &s1);
    assert_eq!(s2.commits.len(), 4);
    assert_eq!(s2.commits[s2.row_of[&root]].parents.len(), 1, "the former root has its parent now");
    same_history(&s2, &load(&clone, &opts).unwrap(), "deepened");
    let _ = std::fs::remove_dir_all(&src);
    let _ = std::fs::remove_dir_all(&clone);
}

#[test]
fn a_grafts_file_turns_reuse_off() {
    let (dir, _) = branchy("reuse-grafts");
    let opts = LoadOptions::all(1000);
    let s1 = load(&dir, &opts).unwrap();
    std::fs::create_dir_all(dir.join(".git/info")).unwrap();
    write(&dir, ".git/info/grafts", "");
    let s2 = reload(&dir, &opts, &s1);
    assert_eq!(s2.commits.len(), s1.commits.len());
    assert!(s2.commits.iter().zip(s1.commits.iter()).all(|(a, b)| !a.author.ptr_eq(&b.author)), "nothing was taken from the earlier load");
    let _ = std::fs::remove_dir_all(&dir);
}
