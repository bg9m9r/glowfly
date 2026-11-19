//! Apply and Unstash with the staged state: `--index` when nothing is staged, the plain form
//! otherwise, and once more after git refuses the staged part.

use super::auto_put_back_tests::{index_wont_fit, names, read, top_oid};
use super::*;
use crate::testkit::git_out;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

const OTHER: &str = "1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b";

#[test]
fn apply_command_is_the_exact_argv() {
    let table: [(bool, bool, &[&str]); 4] = [
        (true, true, &["stash", "pop", "--index", "stash@{3}"]),
        (true, false, &["stash", "pop", "stash@{3}"]),
        (false, true, &["stash", "apply", "--index", "stash@{3}"]),
        (false, false, &["stash", "apply", "stash@{3}"]),
    ];
    for (pop, with_index, want) in table {
        assert_eq!(apply_command(pop, 3, with_index), args(want));
    }
}

/// a.txt staged, b.txt unstaged, c.txt staged "two" and unstaged "three", n.txt staged new and
/// u.txt untracked, all stashed with `push -u`.
pub(crate) fn mixed(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
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
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    write(&dir, "u.txt", "untracked\n");
    git(&dir, &["stash", "push", "-q", "-u", "-m", "wip"]);
    let oid = top_oid(&dir);
    (dir, oid)
}

#[test]
fn the_staged_state_comes_back_staged() {
    for pop in [false, true] {
        let verb = if pop { "pop" } else { "apply" };
        let (dir, oid) = mixed("apply-staged");
        let (back, output) = apply(&dir, pop, 0, oid);
        assert_eq!(back, StashBack::Staged, "{output}");
        assert!(output.starts_with(&format!("$ git stash {verb} --index stash@{{0}}")), "{output}");
        assert!(!came_back_unstaged(&output), "{output}");
        assert_eq!(names(&dir, &["diff", "--cached", "--name-only"]), vec!["a.txt", "c.txt", "n.txt"]);
        assert_eq!(names(&dir, &["diff", "--name-only"]), vec!["b.txt", "c.txt"]);
        assert_eq!(git_out(&dir, &["show", ":c.txt"]).trim_end(), "two");
        assert_eq!(names(&dir, &["ls-files", "--others", "--exclude-standard"]), vec!["u.txt"]);
        let left = list(&dir).unwrap();
        if pop {
            assert!(left.is_empty());
        } else {
            assert_eq!(left.iter().map(|s| s.oid).collect::<Vec<_>>(), vec![oid]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_staged_part_that_no_longer_fits_comes_back_unstaged() {
    for pop in [false, true] {
        let verb = if pop { "pop" } else { "apply" };
        let (dir, oid) = index_wont_fit("apply-unstaged", &[(4, "four")]);
        let (back, output) = apply(&dir, pop, 0, oid);
        assert_eq!(back, StashBack::Unstaged, "{output}");
        assert!(output.contains(&format!("$ git stash {verb} --index stash@{{0}}")), "{output}");
        assert!(output.contains(&format!("$ git stash {verb} stash@{{0}}")), "{output}");
        assert!(output.to_lowercase().contains("without --index"), "{output}");
        assert!(output.ends_with(BACK_UNSTAGED) && came_back_unstaged(&output), "{output}");
        assert!(git_out(&dir, &["diff", "--cached"]).is_empty(), "nothing staged");
        let text = read(&dir, "a.txt");
        assert!(text.contains("four\n") && text.contains("six\n"), "{text}");
        assert_eq!(list(&dir).unwrap().len(), usize::from(!pop));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_fallback_that_conflicts_keeps_the_stash() {
    let (dir, oid) = index_wont_fit("apply-fallback-conflict", &[(4, "four"), (6, "theirs")]);
    let (back, output) = apply(&dir, true, 0, oid);
    assert_eq!(back, StashBack::Conflicted { paths: vec!["a.txt".to_owned()] }, "{output}");
    assert!(!came_back_unstaged(&output), "{output}");
    assert_eq!(list(&dir).unwrap().iter().map(|s| s.oid).collect::<Vec<_>>(), vec![oid]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_working_tree_conflict_runs_one_command_and_keeps_the_stash() {
    let dir = scratch_repo("apply-tree-conflict");
    write(&dir, "a.txt", "one\n");
    write(&dir, "b.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "staged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "b.txt", "mine\n");
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    let oid = top_oid(&dir);
    write(&dir, "b.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "head"]);
    let (back, output) = apply(&dir, true, 0, oid);
    assert_eq!(back, StashBack::Conflicted { paths: vec!["b.txt".to_owned()] }, "{output}");
    assert_eq!(output.lines().filter(|l| l.starts_with("$ git ")).count(), 1, "{output}");
    assert!(output.contains("Index was not unstashed"), "{output}");
    assert_eq!(list(&dir).unwrap().iter().map(|s| s.oid).collect::<Vec<_>>(), vec![oid]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn something_staged_already_runs_the_plain_form() {
    let dir = scratch_repo("apply-other-staged");
    write(&dir, "a.txt", "one\n");
    write(&dir, "x.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "staged\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    let oid = top_oid(&dir);
    write(&dir, "x.txt", "staged\n");
    git(&dir, &["add", "x.txt"]);
    write(&dir, "x.txt", "work\n");
    let (back, output) = apply(&dir, true, 0, oid);
    assert_eq!(back, StashBack::Plain, "{output}");
    assert!(output.starts_with("$ git stash pop stash@{0}"), "{output}");
    assert!(!output.contains("--index"), "{output}");
    assert_eq!(git_out(&dir, &["show", ":x.txt"]).trim_end(), "staged");
    assert_eq!(names(&dir, &["diff", "--cached", "--name-only"]), vec!["x.txt"]);
    assert_eq!(names(&dir, &["diff", "--name-only"]), vec!["a.txt", "x.txt"]);
    assert!(list(&dir).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_moved_list_runs_nothing() {
    let (dir, oid) = mixed("apply-moved");
    let other = Oid::from_str(OTHER).unwrap();
    assert_ne!(other, oid);
    let (back, output) = apply_with(&dir, true, 0, other, &mut |_| panic!("nothing runs"));
    assert_eq!(back, StashBack::Failed);
    assert!(output.contains("no longer the stash that was picked"), "{output}");
    assert_eq!(list(&dir).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_held_command_fails_after_one_run() {
    let (dir, oid) = mixed("apply-held");
    let mut calls = 0;
    let (back, output) = apply_with(&dir, true, 0, oid, &mut |_| {
        calls += 1;
        (false, "held".to_owned())
    });
    assert_eq!((back, output.as_str(), calls), (StashBack::Failed, "held", 1));
    assert_eq!(list(&dir).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_runs_nothing_for_other_failures() {
    let refusal = "error: conflicts in index. Try without --index.";
    let plain = apply_command(true, 0, false);
    let mut never = |_: &[String]| -> (bool, String) { panic!("nothing runs") };

    let (dir, oid) = index_wont_fit("apply-retry-none", &[(4, "four")]);
    assert_eq!(retry_unstaged(&dir, "error: something else", &plain, Some(oid), &mut never), None);
    let other = Oid::from_str(OTHER).unwrap();
    assert_eq!(retry_unstaged(&dir, refusal, &plain, Some(other), &mut never), None);
    // The refusal with the right commit runs the plain command once.
    let mut calls = Vec::new();
    let got = retry_unstaged(&dir, refusal, &plain, Some(oid), &mut |c| {
        calls.push(c.to_vec());
        (true, "ok".to_owned())
    });
    assert_eq!(calls, vec![plain.clone()]);
    assert_eq!(got, Some((true, format!("{refusal}\nok\n{BACK_UNSTAGED}"))));
    let _ = std::fs::remove_dir_all(&dir);

    // A conflicted index is left alone.
    let (dir, oid) = index_wont_fit("apply-retry-conflict", &[(4, "four"), (6, "theirs")]);
    let (back, _) = apply(&dir, true, 0, oid);
    assert!(matches!(back, StashBack::Conflicted { .. }));
    assert_eq!(retry_unstaged(&dir, refusal, &plain, Some(oid), &mut never), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn anything_staged_follows_the_index() {
    let dir = scratch_repo("apply-anything-staged");
    assert!(!anything_staged(&dir), "unborn and empty");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "a.txt"]);
    assert!(anything_staged(&dir), "unborn with a staged file");
    git(&dir, &["commit", "-q", "-m", "first"]);
    assert!(!anything_staged(&dir));
    write(&dir, "a.txt", "two\n");
    assert!(!anything_staged(&dir), "only unstaged");
    git(&dir, &["add", "a.txt"]);
    assert!(anything_staged(&dir));
    let plain = crate::paths::temp_dir().join("glowfly-not-a-repo-apply");
    let _ = std::fs::create_dir_all(&plain);
    assert!(anything_staged(&plain), "doubt counts as staged");
    let _ = std::fs::remove_dir_all(&plain);
    let _ = std::fs::remove_dir_all(&dir);
}
