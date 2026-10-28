//! Putting an auto-stash back with `git stash pop --index`: the staged state survives, and a
//! plain pop follows when git can't restore it.

use super::*;
use crate::testkit::git_out;
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

fn names(dir: &Path, a: &[&str]) -> Vec<String> {
    git_out(dir, a).lines().map(str::to_owned).collect()
}

fn top_oid(dir: &Path) -> Oid {
    list(dir).unwrap()[0].oid
}

#[test]
fn put_back_keeps_staged_and_unstaged_apart() {
    let dir = scratch_repo("auto-put-back-index");
    for f in ["a.txt", "b.txt", "c.txt"] {
        write(&dir, f, "one\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "staged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "b.txt", "unstaged\n");
    write(&dir, "c.txt", "two\n");
    git(&dir, &["add", "c.txt"]);
    write(&dir, "c.txt", "three\n");
    git(&dir, &["stash", "push", "-q", "-m", "Auto-stash before checking out x"]);
    let oid = top_oid(&dir);
    assert_eq!(prepare_put_back(&dir, oid), Ok(0));
    let (outcome, output) = pop_put_back(&dir, 0, oid);
    assert_eq!(outcome, PutBack::Restored, "{output}");
    assert!(output.starts_with("$ git stash pop --index stash@{0}"), "{output}");
    assert_eq!(names(&dir, &["diff", "--cached", "--name-only"]), vec!["a.txt", "c.txt"]);
    assert_eq!(names(&dir, &["diff", "--name-only"]), vec!["b.txt", "c.txt"]);
    assert_eq!(git_out(&dir, &["show", ":c.txt"]).trim_end(), "two");
    assert_eq!(read(&dir, "c.txt"), "three\n");
    assert!(list(&dir).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lines 1..9, each on its own line, with the `change`d ones replaced.
fn lines(change: &[(usize, &str)]) -> String {
    (1..=9)
        .map(|n| {
            let line = change.iter().find(|(i, _)| *i == n).map_or(n.to_string(), |(_, t)| (*t).to_owned());
            line + "\n"
        })
        .collect()
}

/// a.txt holds the lines 1..9; line 6 is changed to "six", staged and stashed. HEAD then gets
/// `head`: line 4 sits inside the staged hunk's context, so `git apply --cached` fails.
fn index_wont_fit(name: &str, head: &[(usize, &str)]) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", &lines(&[]));
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", &lines(&[(6, "six")]));
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["stash", "push", "-q", "-m", "Auto-stash before checking out x"]);
    let oid = top_oid(&dir);
    write(&dir, "a.txt", &lines(head));
    git(&dir, &["commit", "-q", "-am", "head"]);
    (dir, oid)
}

#[test]
fn put_back_falls_back_when_the_index_cant_be_restored() {
    let (dir, oid) = index_wont_fit("auto-put-back-fallback", &[(4, "four")]);
    let (outcome, output) = pop_put_back(&dir, 0, oid);
    assert_eq!(outcome, PutBack::RestoredUnstaged, "{output}");
    let text = read(&dir, "a.txt");
    assert!(text.contains("four\n") && text.contains("six\n"), "{text}");
    assert!(git_out(&dir, &["diff", "--cached"]).is_empty(), "nothing staged");
    assert_eq!(names(&dir, &["diff", "--name-only"]), vec!["a.txt"]);
    assert!(list(&dir).unwrap().is_empty());
    assert!(output.contains("$ git stash pop --index stash@{0}"), "{output}");
    assert!(output.contains("$ git stash pop stash@{0}"), "{output}");
    assert!(output.to_lowercase().contains("without --index"), "{output}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fallback_that_conflicts_keeps_the_stash() {
    let (dir, oid) = index_wont_fit("auto-put-back-fallback-conflict", &[(4, "four"), (6, "theirs")]);
    let (outcome, output) = pop_put_back(&dir, 0, oid);
    assert_eq!(outcome, PutBack::Conflicted { paths: vec!["a.txt".to_owned()] }, "{output}");
    let left = list(&dir).unwrap();
    assert_eq!((left.len(), left[0].index, left[0].oid), (1, 0, oid), "the stash stays");
    let _ = std::fs::remove_dir_all(&dir);
}
