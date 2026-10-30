//! Which commands move the work tree, the batch update and edit command lines, and the edit
//! commands run against a real submodule.

use super::*;
use crate::gitcli;
use crate::testutil::*;
use std::path::Path;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn work_tree_moving_commands() {
    let moving: &[&[&str]] = &[
        &["checkout", "main", "--"],
        &["checkout", "--track", "origin/x", "--"],
        &["checkout", "-b", "n"],
        &["switch", "x"],
        &["pull", "--ff"],
        &["merge", "x"],
        &["rebase", "main"],
        &["reset", "--hard", "HEAD~1"],
        &["reset", "--mixed", "x"],
        &["-c", "k=v", "pull"],
        &["--literal-pathspecs", "-C", "dir", "checkout", "main"],
        &["cherry-pick", "abc1234"],
        &["cherry-pick", "--continue"],
        &["revert", "--no-edit", "abc1234"],
        &["revert", "--skip"],
        &["stash", "pop", "stash@{0}"],
        &["stash", "apply", "stash@{1}"],
        &["stash", "pop", "--index", "stash@{0}"],
    ];
    for c in moving {
        assert!(moves_work_tree(&[s(c)]), "{c:?}");
    }
    assert!(moves_work_tree(&crate::undo::hard_reset_commands("HEAD~1")));
    let still: &[&[&str]] = &[
        &["checkout", "--", "a.txt"],
        &["checkout", "--theirs", "--", "a.txt"],
        &["checkout", "-p"],
        &["reset", "--soft", "x"],
        &["reset", "--", "a"],
        &["commit", "-m", "x"],
        &["fetch", "--all"],
        &["submodule", "update", "--init", "--recursive"],
        &["-c", "k=v"],
        &["stash", "push", "-u"],
        &["stash", "drop", "stash@{0}"],
        &["stash"],
    ];
    for c in still {
        assert!(!moves_work_tree(&[s(c)]), "{c:?}");
    }
    assert!(!moves_work_tree(&[]));
}

#[test]
fn commits_that_finish_a_stopped_operation() {
    use git2::RepositoryState::*;
    let commit = [s(&["commit", "-F", "msg"])];
    for state in [Merge, CherryPick, CherryPickSequence, Revert, RevertSequence] {
        assert!(finishes_operation(state, &commit), "{state:?}");
        assert!(!finishes_operation(state, &[s(&["add", "a.txt"])]), "{state:?}");
    }
    for state in [Clean, Rebase, RebaseInteractive, Bisect] {
        assert!(!finishes_operation(state, &commit), "{state:?}");
    }
    assert!(finishes_operation(Merge, &[s(&["add", "--all"]), s(&["commit", "-F", "msg"])]), "stage all and commit");
}

#[test]
fn undos_that_move_the_work_tree() {
    use crate::undo::{Action, Changes, HeadPos, MoveOp, ResetMode};
    let o = git2::Oid::zero();
    let reset = |mode| Action::Reset { mode, branch: None, from: o, to: o, stash: None };
    let moving = [
        Action::Checkout { from: HeadPos::Detached(o), to: HeadPos::Branch("x".into()), created: None },
        reset(ResetMode::Hard),
        reset(ResetMode::Mixed),
        Action::Moved { op: MoveOp::CherryPick, detail: None, branch: None, from: o, to: o },
        Action::Moved { op: MoveOp::Revert, detail: None, branch: None, from: o, to: o },
        Action::Restacked { detail: "restack".into(), moves: Vec::new(), partial: false },
        Action::StashPush { stash: o, head: o, message: "m".into(), replay: None },
        Action::StashApply { pop: true, stash: o, head: o, message: "m".into(), left: Changes(Vec::new()) },
        Action::DiscardAll { stash: o, head: o },
    ];
    for a in &moving {
        assert!(undo_moves_work_tree(a, false) && undo_moves_work_tree(a, true), "{a:?}");
    }
    let still = [
        reset(ResetMode::Soft),
        Action::Commit { branch: None, from: o, to: o, message: "m".into(), amend: false },
        Action::CreateBranch { name: "b".into(), oid: o, upstream: None },
        Action::CreateTag { name: "t".into(), oid: o },
        Action::StashDrop { stash: o, message: "m".into() },
    ];
    for a in &still {
        assert!(!undo_moves_work_tree(a, false) && !undo_moves_work_tree(a, true), "{a:?}");
    }
    let kept = Action::AbortKept { op: MoveOp::Merge, saved: o, head: o, branch: None, left: Changes(Vec::new()) };
    assert!(!undo_moves_work_tree(&kept, false), "undoing the abort puts the merge back in progress");
    assert!(undo_moves_work_tree(&kept, true), "redoing it aborts again");
}

#[test]
fn batch_update_command_lines() {
    assert_eq!(batch_update_cmds(None, true), [s(&["submodule", "update", "--init", "--recursive", "--checkout"])]);
    assert_eq!(batch_update_cmds(None, false), [s(&["submodule", "update", "--init", "--recursive"])]);
    let paths = vec!["a".to_owned(), "lib[1]".to_owned()];
    assert_eq!(
        batch_update_cmds(Some(&paths), false),
        [s(&["submodule", "update", "--init", "--recursive", "--", ":(literal)a", ":(literal)lib[1]"])]
    );

    let long: Vec<String> = (0..60).map(|i| format!("{i:03}-{}", "x".repeat(1000))).collect();
    let cmds = batch_update_cmds(Some(&long), true);
    assert!(cmds.len() > 1, "split into several commands");
    let mut seen = Vec::new();
    for c in &cmds {
        let dash = c.iter().position(|a| a == "--").expect("pathspecs after --");
        assert_eq!(&c[..dash], s(&["submodule", "update", "--init", "--recursive", "--checkout"]));
        let specs = &c[dash + 1..];
        let bytes: usize = specs.iter().map(|p| p.len() + 1).sum::<usize>() - 1;
        assert!(bytes <= BATCH_PATH_BYTES, "{bytes}");
        seen.extend(specs.iter().map(|p| p.strip_prefix(":(literal)").unwrap().to_owned()));
    }
    assert_eq!(seen, long, "every path once, in order");
}

#[test]
fn edit_command_lines() {
    assert_eq!(
        set_url_cmds("libs/lib", " https://h/o/lib.git "),
        [s(&["submodule", "set-url", "--", "libs/lib", "https://h/o/lib.git"]), s(&["submodule", "sync", "--", ":(literal)libs/lib"])]
    );
    assert_eq!(set_branch_cmd("libs/lib", Some("dev")), s(&["submodule", "set-branch", "--branch", "dev", "--", "libs/lib"]));
    assert_eq!(set_branch_cmd("libs/lib", None), s(&["submodule", "set-branch", "--default", "--", "libs/lib"]));
    assert_eq!(check_edit_branch(" . "), Ok(Some(".".to_owned())));
    assert_eq!(check_edit_branch(" dev "), Ok(Some("dev".to_owned())));
    assert_eq!(check_edit_branch(""), Ok(None));
    assert_eq!(check_edit_branch("bad..name"), Err(()));
}

/// Whether this git can run `submodule set-url`; says so when it can't.
fn edit_supported(dir: &Path) -> bool {
    let ok = gitcli::version(dir).is_some_and(|v| (v.0, v.1) >= crate::git_version::Feature::SubmoduleEdit.min());
    if !ok {
        eprintln!("git is older than 2.25, skipping");
    }
    ok
}

fn run_all(dir: &Path, cmds: &[Vec<String>]) {
    for c in cmds {
        let (ok, out) = gitcli::run(dir, c);
        assert!(ok, "{c:?}: {out}");
    }
}

fn cleanup(parent: &Path) {
    let stem = parent.file_name().unwrap().to_string_lossy().into_owned();
    let pid = format!("-{}", std::process::id());
    let lib = parent.with_file_name(format!("{}-lib{pid}", stem.strip_suffix(&pid).unwrap_or(&stem)));
    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn set_url_writes_gitmodules_config_and_origin() {
    let (parent, sub) = with_submodule("sm-set-url");
    if edit_supported(&parent) {
        let url = "https://example.com/o/other-lib.git";
        run_all(&parent, &set_url_cmds("libs/lib", url));
        let gitmodules = git2::Config::open(&parent.join(".gitmodules")).unwrap();
        assert_eq!(gitmodules.get_string("submodule.libs/lib.url").unwrap(), url);
        let config = git2::Repository::open(&parent).unwrap().config().unwrap().snapshot().unwrap();
        assert_eq!(config.get_string("submodule.libs/lib.url").unwrap(), url);
        let inner = git2::Repository::open(&sub).unwrap();
        assert_eq!(inner.find_remote("origin").unwrap().url(), Some(url));
    }
    cleanup(&parent);
}

#[test]
fn set_branch_writes_and_clears_the_branch() {
    let (parent, _sub) = with_submodule("sm-set-branch");
    if edit_supported(&parent) {
        let branch = || git2::Config::open(&parent.join(".gitmodules")).unwrap().get_string("submodule.libs/lib.branch").ok();
        run_all(&parent, &[set_branch_cmd("libs/lib", Some("dev"))]);
        assert_eq!(branch().as_deref(), Some("dev"));
        run_all(&parent, &[set_branch_cmd("libs/lib", None)]);
        assert_eq!(branch(), None);
    }
    cleanup(&parent);
}

#[test]
fn batch_update_initializes_again() {
    let (parent, _sub) = with_submodule("sm-batch-update");
    git(&parent, &["submodule", "deinit", "-q", "-f", "libs/lib"]);
    let state = || crate::repo::submodules_at(&parent).unwrap()[0].state;
    assert_eq!(state(), crate::repo::SubmoduleState::Uninitialized);
    run_all(&parent, &batch_update_cmds(None, true));
    assert_eq!(state(), crate::repo::SubmoduleState::InSync);
    cleanup(&parent);
}
