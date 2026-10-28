//! Aborting a merge, cherry-pick or revert while keeping its work, and Undo putting the
//! operation back in progress with every resolution, conflict and edit.

use super::tests::commit;
use super::*;
use crate::gitcli::run;
use crate::repo::status_at;
use crate::testutil::*;
use git2::{Repository, RepositoryState};
use std::path::Path;

pub(super) fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

pub(super) fn state(dir: &Path) -> RepositoryState {
    Repository::open(dir).unwrap().state()
}

/// The git folder's `name` file, if any.
pub(super) fn git_file(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(".git").join(name)).ok()
}

/// Paths with conflict stages in the index, sorted.
pub(super) fn conflicted(dir: &Path) -> Vec<String> {
    let index = Repository::open(dir).unwrap().index().unwrap();
    let mut out: Vec<String> =
        index.conflicts().unwrap().flatten().filter_map(|c| c.our.or(c.their)).map(|e| String::from_utf8(e.path).unwrap()).collect();
    out.sort();
    out
}

/// The staged rows as (path, status, renamed from).
pub(super) fn staged(dir: &Path) -> Vec<(String, char, Option<String>)> {
    let (_, staged, _) = status_at(dir, true).unwrap();
    staged.into_iter().map(|f| (f.path, f.status, f.renamed_from)).collect()
}

pub(super) fn kept_refs(dir: &Path) -> usize {
    let repo = Repository::open(dir).unwrap();
    repo.references_glob(&format!("{ABORT_REF_PREFIX}*")).unwrap().count()
}

/// Runs `cmds` in order through gitcli, stopping at the first that fails, as the job does.
pub(super) fn run_all(dir: &Path, cmds: &[Vec<String>]) -> (bool, String) {
    let mut out = String::new();
    for c in cmds {
        let (ok, text) = run(dir, c);
        out.push_str(&text);
        if !ok {
            return (false, out);
        }
    }
    (true, out)
}

/// Aborts keeping the work, running the commands as the job does.
pub(super) fn abort(dir: &Path) -> Action {
    let r = abort_keeping(dir, Some(&[]), |cmds| run_all(dir, cmds));
    assert!(r.ok, "{}", r.output);
    assert!(r.output.contains(ABORT_REF_PREFIX), "names the ref: {}", r.output);
    r.action.expect("recorded for undo")
}

pub(super) fn undo(dir: &Path, action: &Action) {
    assert_eq!(plan(dir, action, false).unwrap(), Plan::RestoreAbort);
    restore_abort(dir, action).unwrap();
}

/// main and other both change a.txt and b.txt; other also adds n.txt. Stopped merging other.
pub(super) fn two_conflicts(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "b.txt", "b base\n");
    write(&dir, "u.txt", "unrelated\n");
    commit(&dir, "a.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "b.txt", "b theirs\n");
    write(&dir, "n.txt", "new\n");
    commit(&dir, "a.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "b.txt", "b ours\n");
    commit(&dir, "a.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge stops for the conflicts");
    dir
}

#[test]
fn merge_with_conflicts_left_comes_back_in_progress() {
    let dir = two_conflicts("abort-keep-partial");
    write(&dir, "a.txt", "merged\n");
    git(&dir, &["add", "a.txt"]);
    let edited = read(&dir, "b.txt").replace("b ours", "b ours, edited");
    write(&dir, "b.txt", &edited);
    write(&dir, "u.txt", "unrelated, mine\n");
    let merge_head = git_file(&dir, "MERGE_HEAD").unwrap();
    let merge_msg = git_file(&dir, "MERGE_MSG").unwrap();

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of merge");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!((read(&dir, "a.txt"), read(&dir, "b.txt")), ("ours\n".into(), "b ours\n".into()));
    assert!(!dir.join("n.txt").exists(), "the merge's new file goes");
    assert_eq!(read(&dir, "u.txt"), "unrelated, mine\n", "the unrelated edit stays in place");
    assert_eq!(kept_refs(&dir), 1);
    let Action::AbortKept { saved, .. } = &action else { panic!("a kept abort") };
    let repo = Repository::open(&dir).unwrap();
    let parents: Vec<_> = repo.find_commit(*saved).unwrap().parent_ids().collect();
    assert_eq!(parents[1].to_string(), merge_head.trim(), "the merged commit stays reachable");

    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::Merge);
    assert_eq!(git_file(&dir, "MERGE_HEAD").unwrap(), merge_head);
    assert_eq!(git_file(&dir, "MERGE_MSG").unwrap(), merge_msg);
    assert_eq!(read(&dir, "a.txt"), "merged\n");
    assert_eq!(read(&dir, "b.txt"), edited, "the conflicted file's edits come back");
    assert_eq!(read(&dir, "n.txt"), "new\n");
    assert_eq!(read(&dir, "u.txt"), "unrelated, mine\n");
    assert_eq!(conflicted(&dir), ["b.txt"], "still conflicted");
    let staged = staged(&dir);
    assert!(staged.iter().any(|(p, s, _)| p == "a.txt" && *s == 'M'), "{staged:?}");
    assert!(staged.iter().any(|(p, s, _)| p == "n.txt" && *s == 'A'), "{staged:?}");
    assert_eq!(kept_refs(&dir), 0, "the ref goes once the work is back");

    // The merge carries on from there.
    write(&dir, "b.txt", "b merged\n");
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "--no-edit"]);
    assert_eq!(state(&dir), RepositoryState::Clean);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_pick_with_a_rename_and_unrelated_edits() {
    let dir = scratch_repo("abort-keep-pick");
    write(&dir, "old.txt", "moved\n");
    write(&dir, "u.txt", "unrelated\n");
    commit(&dir, "a.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let pick = commit(&dir, "a.txt", "theirs\n", "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "ours\n", "ours");
    let (ok, _) = run(&dir, &args(&["cherry-pick", &pick.to_string()]));
    assert!(!ok, "the pick stops for the conflict");
    assert_eq!(state(&dir), RepositoryState::CherryPick);
    write(&dir, "u.txt", "unrelated, mine\n");
    write(&dir, "scratch.txt", "untracked\n");

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of cherry-pick");
    assert_eq!(state(&dir), RepositoryState::Clean);
    assert_eq!(read(&dir, "old.txt"), "moved\n", "the rename's old path comes back");
    assert!(!dir.join("new.txt").exists());
    assert_eq!(read(&dir, "u.txt"), "unrelated, mine\n");
    assert_eq!(read(&dir, "scratch.txt"), "untracked\n");

    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::CherryPick);
    assert_eq!(git_file(&dir, "CHERRY_PICK_HEAD").unwrap().trim(), pick.to_string());
    assert_eq!(conflicted(&dir), ["a.txt"]);
    assert!(read(&dir, "a.txt").contains("<<<<<<<"), "the markers come back");
    assert!(!dir.join("old.txt").exists());
    assert!(staged(&dir).contains(&("new.txt".into(), 'R', Some("old.txt".into()))), "{:?}", staged(&dir));
    assert_eq!(read(&dir, "u.txt"), "unrelated, mine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_resolved_comes_back_and_redo_aborts_again() {
    let dir = scratch_repo("abort-keep-revert");
    commit(&dir, "a.txt", "one\n", "one");
    let two = commit(&dir, "a.txt", "two\n", "two");
    commit(&dir, "a.txt", "three\n", "three");
    let (ok, _) = run(&dir, &args(&["revert", "--no-edit", &two.to_string()]));
    assert!(!ok, "the revert stops for the conflict");
    assert_eq!(state(&dir), RepositoryState::Revert);
    write(&dir, "a.txt", "one and three\n");
    git(&dir, &["add", "a.txt"]);

    let action = abort(&dir);
    assert_eq!(action.describe(), "abort of revert");
    assert_eq!(read(&dir, "a.txt"), "three\n");
    undo(&dir, &action);
    assert_eq!(state(&dir), RepositoryState::Revert);
    assert_eq!(git_file(&dir, "REVERT_HEAD").unwrap().trim(), two.to_string());
    assert_eq!(read(&dir, "a.txt"), "one and three\n");
    assert!(staged(&dir).iter().any(|(p, _, _)| p == "a.txt"));

    assert_eq!(plan(&dir, &action, true).unwrap(), Plan::AbortAgain);
    let again = abort(&dir);
    assert_eq!(state(&dir), RepositoryState::Clean);
    undo(&dir, &again);
    assert_eq!(read(&dir, "a.txt"), "one and three\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_refuses_once_the_tree_moved_on() {
    let dir = two_conflicts("abort-keep-moved");
    let action = abort(&dir);
    write(&dir, "a.txt", "edited after the abort\n");
    let Plan::Refuse(Refusal::Dirty(why)) = plan(&dir, &action, false).unwrap() else { panic!("an edit in the way") };
    assert!(why.contains("abort of merge"), "{why}");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    write(&dir, "n.txt", "something new where the merge's file was\n");
    assert!(matches!(plan(&dir, &action, false).unwrap(), Plan::Refuse(Refusal::Dirty(_))), "a file at a path the merge added");
    std::fs::remove_file(dir.join("n.txt")).unwrap();
    write(&dir, "u.txt", "an unrelated edit doesn't matter\n");
    assert_eq!(plan(&dir, &action, false).unwrap(), Plan::RestoreAbort);

    commit(&dir, "u.txt", "moved on\n", "later");
    let Plan::Refuse(r) = plan(&dir, &action, false).unwrap() else { panic!("HEAD moved") };
    assert!(r.is_stale() && r.message().contains(ABORT_REF_PREFIX), "{r:?}");
    assert_eq!(kept_refs(&dir), 1, "the kept work stays for recovery");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_aborts_when_the_work_cant_be_kept() {
    let dir = two_conflicts("abort-keep-refused");
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    std::fs::create_dir(dir.join("b.txt")).unwrap();
    write(&dir, "b.txt/inside.txt", "a folder where the conflicted file was\n");
    let r = abort_keeping(&dir, Some(&[]), |_| panic!("nothing runs"));
    assert!(!r.ok && r.action.is_none());
    assert!(r.output.starts_with("Nothing was aborted") && r.output.contains("b.txt is a folder holding b.txt/inside.txt"), "{}", r.output);
    assert_eq!(state(&dir), RepositoryState::Merge, "nothing was aborted");
    assert_eq!(kept_refs(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_commands_split_long_lists() {
    let paths: Vec<String> = (0..3000).map(|i| format!("folder/file-{i:05}.txt")).collect();
    let cmds = aborted::path_commands(&["checkout", "--"], &paths);
    assert!(cmds.len() > 1);
    assert!(cmds.iter().all(|c| c[..2] == ["checkout", "--"]));
    assert_eq!(cmds.iter().map(|c| c.len() - 2).sum::<usize>(), paths.len());
    assert!(aborted::path_commands(&["checkout", "--"], &[]).is_empty());
}
