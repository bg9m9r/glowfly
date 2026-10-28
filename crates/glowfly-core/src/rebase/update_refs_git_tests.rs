//! `rebase.updateRefs` predicted exactly as git writes it: branch aliases (symbolic refs), the
//! order of a commit's lines, branches other worktrees hold (also through a rebase there, and
//! not the HEAD of a bare repository), names that aren't UTF-8, and why a list is refused when
//! git leaves a branch out.

use super::plan_tests::linear;
use super::run_tests::state;
use super::update_refs::refs_problem;
use super::update_refs_tests::{git_dir, gits_todo, installs, kinds, plan_from, stacked, unsupported};
use super::*;
use crate::testkit::git_out;
use crate::testutil::*;
use git2::RepositoryState;
use std::path::{Path, PathBuf};

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let p = dir.with_file_name(format!("{}-{suffix}", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// git's own list for `git rebase -i <from>~1`, saved by an editor that then fails.
fn raw_todo(dir: &Path, from: Oid) -> String {
    let saved = git_dir(dir).join("raw-todo");
    let editor = format!("sh -c 'cp \"$1\" {} && exit 1' sh", shell_quote(&saved.to_string_lossy()));
    let upstream = format!("{from}~1");
    let _ = crate::gitcli::run_with_input(dir, &crate::gitcli::args(&["rebase", "-i", &upstream]), "", &[("GIT_SEQUENCE_EDITOR", &editor)]);
    let _ = crate::gitcli::run(dir, &crate::gitcli::args(&["rebase", "--abort"]));
    String::from_utf8_lossy(&std::fs::read(&saved).unwrap()).into_owned()
}

/// The update-ref lines of the list the plan's steps make, in order.
fn our_refs(dir: &Path, steps: &[TodoStep]) -> Vec<String> {
    listed_refs(&build(steps, &git_dir(dir), false).0)
}

/// zeta and an alias of lower on lower's commit, beside it: git lists the alias, not its
/// target, and a commit's lines in reverse name order.
#[test]
fn an_alias_is_listed_in_place_of_its_target_in_gits_order() {
    let (dir, [_, c2, ..]) = stacked("update-refs-alias");
    git(&dir, &["branch", "zeta", &c2.to_string()]);
    git(&dir, &["symbolic-ref", "refs/heads/sym2", "refs/heads/lower"]);
    let p = plan_from(&dir, c2);
    assert!(p.held.is_empty());
    assert_eq!(kinds(&p.steps)[..4], ["pick c2", "update-ref zeta", "update-ref sym2", "pick c3"]);
    let theirs = gits_todo(&dir, &p);
    assert_eq!(listed_refs(&theirs), our_refs(&dir, &p.steps), "{theirs}");
    installs(&dir, &theirs, &p.steps).expect("the plan's list matches git's");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An alias of a branch checked out in another worktree: git would list the alias and move
/// that branch, so the plan, Start and the graph's edits refuse it.
#[test]
fn an_alias_of_a_branch_checked_out_elsewhere_holds_the_rebase() {
    let (dir, [_, c2, c3, _]) = stacked("update-refs-alias-busy");
    let wt = sibling(&dir, "wt");
    git(&dir, &["worktree", "add", "-q", &wt.to_string_lossy(), "mid"]);
    git(&dir, &["symbolic-ref", "refs/heads/alias", "refs/heads/mid"]);
    let p = plan_from(&dir, c2);
    let held = HeldAlias { alias: "alias".into(), target: "mid".into(), elsewhere: true };
    assert_eq!(p.held, std::slice::from_ref(&held));
    assert!(listed_refs(&raw_todo(&dir, c2)).contains(&"refs/heads/alias".to_owned()), "git would follow the alias");
    let req = StartRequest { head: p.head, upstream: p.upstream, items: p.steps.clone(), autostash: false };
    assert_eq!(unsupported(start(&dir, &req, &SequenceEditor::Copy)), held.text());
    assert_eq!(unsupported(prepare_edit(&dir, &QuickEdit::Reword(c3))), held.text());
    assert_eq!(git_out(&dir, &["rev-parse", "mid"]).trim(), c3.to_string(), "mid stays");

    // An alias of a ref that isn't a local branch.
    git(&dir, &["symbolic-ref", "-d", "refs/heads/alias"]);
    git(&dir, &["update-ref", "refs/remotes/origin/x", &c3.to_string()]);
    git(&dir, &["symbolic-ref", "refs/heads/remote-alias", "refs/remotes/origin/x"]);
    let p = plan_from(&dir, c2);
    assert_eq!(p.held, [HeldAlias { alias: "remote-alias".into(), target: "refs/remotes/origin/x".into(), elsewhere: false }]);
    git(&dir, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A branch another worktree's stopped rebase will move is held there, as git sees it.
#[test]
fn a_branch_another_worktrees_rebase_moves_is_left_alone() {
    let (dir, [c1, c2, c3, c4]) = stacked("update-refs-other-rebase");
    let wt = sibling(&dir, "wt");
    git(&dir, &["worktree", "add", "-q", "-b", "top", &wt.to_string_lossy(), &c4.to_string()]);
    let todo = format!("edit {c2} c2\nupdate-ref refs/heads/lower\npick {c3} c3\npick {c4} c4\n");
    assert!(super::remaining_tests::rebase_i(&wt, &c1.to_string(), &todo));
    assert!(git_out(&wt, &["status"]).contains("rebase in progress"));
    let p = plan_from(&dir, c2);
    assert!(p.steps.iter().all(|s| s.update_ref() != Some("lower")), "{:?}", kinds(&p.steps));
    installs(&dir, &gits_todo(&dir, &p), &p.steps).expect("the plan's list matches git's");
    git(&wt, &["rebase", "--abort"]);
    git(&dir, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// In a worktree of a bare repository, the bare HEAD's branch isn't checked out anywhere.
#[test]
fn a_bare_repositorys_head_branch_is_not_held() {
    let (dir, [_, c2, ..]) = linear("update-refs-bare");
    let bare = sibling(&dir, "bare");
    git(&dir, &["clone", "-q", "--bare", &dir.to_string_lossy(), &bare.to_string_lossy()]);
    let wt = sibling(&dir, "bare-wt");
    git(&bare, &["worktree", "add", "-q", "-b", "work", &wt.to_string_lossy(), "main"]);
    set_config(&wt, &[("rebase.updateRefs", "true"), ("user.name", "Test"), ("user.email", "test@example.com")]);
    let p = plan_from(&wt, c2);
    assert_eq!(kinds(&p.steps).last().map(String::as_str), Some("update-ref main"), "{:?}", kinds(&p.steps));
    let theirs = gits_todo(&wt, &p);
    assert_eq!(listed_refs(&theirs), our_refs(&wt, &p.steps), "{theirs}");
    assert_eq!(state(&wt), RepositoryState::Clean);
    for d in [&wt, &bare, &dir] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A branch name that isn't UTF-8 on a commit of the range is refused by name, not left out.
#[test]
fn a_branch_name_that_isnt_utf8_is_refused() {
    let (dir, [_, c2, c3, _]) = stacked("update-refs-utf8");
    let packed = git_dir(&dir).join("packed-refs");
    let mut bytes = std::fs::read(&packed).unwrap_or_else(|_| b"# pack-refs with: peeled fully-peeled sorted \n".to_vec());
    bytes.extend_from_slice(format!("{c3} refs/heads/caf").as_bytes());
    bytes.extend_from_slice(b"\xe9\n");
    std::fs::write(&packed, bytes).unwrap();
    match plan(&dir, &RebaseBase::FromCommit(c2)) {
        Err(crate::error::Error::Unsupported(t)) => assert_eq!(t, NOT_UTF8),
        // libgit2 can't read the ref and the plan passes over it: git's list then has a line the
        // plan's lacks, which Start refuses by the same text.
        Ok(p) => assert_eq!(unsupported(installs(&dir, &gits_todo(&dir, &p), &p.steps)), NOT_UTF8),
        Err(e) => panic!("{e:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_git_leaves_out_is_named_with_its_reason() {
    let theirs = "pick abc c2\n# Ref refs/heads/mid checked out at '/work/wt'\npick def c3\n";
    let ours = "pick abc c2\nupdate-ref refs/heads/mid\npick def c3\n";
    let why = refs_problem(theirs, ours).unwrap();
    assert!(why.starts_with("git won't move mid (checked out at '/work/wt')"), "{why}");
    assert!(!why.contains("reload"), "reloading wouldn't help: {why}");
    assert_eq!(refs_problem(ours, theirs).as_deref(), Some(REFS_CHANGED), "git moving one the list doesn't");
    assert_eq!(refs_problem(ours, ours), None);
}
