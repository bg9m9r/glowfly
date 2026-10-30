//! A job that stopped on conflicts counts its files from git's CONFLICT lines, then from the
//! first status read after it: each conflicted path once, with View conflicts on a path the
//! status lists. That read ends the wait whatever it found, and so does the next job.

use super::auto_stash_tests::up;
use super::failure_headline_tests::{last_toast, opened, running};
use super::tests::{finish, git};
use crate::logs::LogsMsg;
use crate::msg::{ConflictMsg, JobMsg, LoadMsg, Msg};
use crate::state::AppState;
use glowfly_core::gitcli::{args, run_with_input};
use std::path::Path;

const FOUR: &str = "Stopped on conflicts in 4 files. Resolve them, then continue or abort.";

/// `dir` stopped in a merge where side renamed a.txt to `to` (deleted on main) and both renamed
/// r.txt: `to`, r.txt, r1.txt and r2.txt are conflicted. Returns git's rename/delete line.
fn rename_conflicts(dir: &Path, to: &str) -> String {
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("r.txt"), "x\ny\nz\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "base"]);
    git(dir, &["checkout", "-q", "-b", "side"]);
    if let Some(parent) = Path::new(to).parent() {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    git(dir, &["mv", "a.txt", to]);
    git(dir, &["mv", "r.txt", "r1.txt"]);
    git(dir, &["commit", "-q", "-m", "side"]);
    git(dir, &["checkout", "-q", "main"]);
    git(dir, &["rm", "-q", "a.txt"]);
    git(dir, &["mv", "r.txt", "r2.txt"]);
    git(dir, &["commit", "-q", "-m", "main"]);
    let (ok, out) = run_with_input(dir, &args(&["merge", "side"]), "", &[]);
    assert!(!ok, "{out}");
    out.lines().find(|l| l.starts_with("CONFLICT (rename/delete)")).expect("a rename/delete line").to_owned()
}

/// The merge's job ends with only `line` of git's CONFLICT lines, as when git words the others
/// in a way Glowfly can't read. Returns the effects its end asked for (the refresh).
fn merge_ended(st: &mut AppState, dir: &Path, line: &str) -> Vec<crate::effect::Effect> {
    running(st, dir, "Merge");
    let output = format!("$ git merge side\n{line}\nAutomatic merge failed; fix conflicts and then commit the result.");
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }))
}

/// What the conflicts toast's fix opens, if it has one.
fn view_target(st: &AppState) -> Option<String> {
    let t = st.toasts.iter().find(|t| t.text.starts_with("Stopped on conflicts"))?;
    let msg = &t.action.as_ref()?.msg;
    let msg = match **msg {
        Msg::Logs(LogsMsg::Fix(id)) => &st.logs.fix(id)?.msg,
        _ => msg,
    };
    match &**msg {
        Msg::Conflict(ConflictMsg::OpenConflict(p)) => Some(p.clone()),
        _ => None,
    }
}

#[test]
fn the_first_status_read_counts_each_path_once() {
    let (mut st, dir) = opened("cc-counted");
    let line = rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, &line);
    assert!(last_toast(&st).starts_with("Stopped on conflicts in 1 file."), "{}", last_toast(&st));
    assert_eq!(view_target(&st).as_deref(), Some("b.txt"));
    finish(&mut st, fx);
    assert!(st.toasts.iter().any(|t| t.text == FOUR));
    let run = st.log.iter().rev().find_map(|e| e.run.as_ref()).expect("the merge's run");
    assert_eq!(run.summary, FOUR);
    assert_eq!(view_target(&st).as_deref(), Some("b.txt"), "git's first path, which the status lists");
    assert!(st.logs.recount.is_none(), "counted once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_first_read_ends_the_wait() {
    let (mut st, dir) = opened("cc-failed");
    let line = rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, &line);
    let failed = LoadMsg::RepoUnchanged {
        generation: st.load_gen,
        result: Err("could not read the status".into()),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    up(&mut st, Msg::Load(failed));
    assert!(st.logs.recount.is_none(), "the first read ended it");
    // A later read that works rewrites nothing.
    finish(&mut st, fx);
    assert!(st.toasts.iter().any(|t| t.text.starts_with("Stopped on conflicts in 1 file.")));
    assert!(!st.toasts.iter().any(|t| t.text == FOUR));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_next_job_ends_the_wait() {
    let (mut st, dir) = opened("cc-next-job");
    let line = rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, &line);
    running(&mut st, &dir, "Fetch");
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(st.logs.recount.is_none());
    finish(&mut st, fx);
    assert!(!st.toasts.iter().any(|t| t.text == FOUR));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A new name with " in " in it can be read two ways: it is counted but not named (no View
/// conflicts on a cut-short path) until the status tells which reading it is.
#[test]
fn a_path_read_two_ways_is_named_by_the_status() {
    let (mut st, dir) = opened("cc-unsure");
    let line = rename_conflicts(&dir, "docs/log in to it.md");
    let fx = merge_ended(&mut st, &dir, &line);
    assert!(last_toast(&st).starts_with("Stopped on conflicts in 1 file."), "{}", last_toast(&st));
    assert_eq!(view_target(&st), None);
    finish(&mut st, fx);
    assert!(st.toasts.iter().any(|t| t.text == FOUR));
    assert_eq!(view_target(&st).as_deref(), Some("docs/log in to it.md"));
    let t = st.toasts.iter().find(|t| t.text == FOUR).unwrap();
    assert!(t.details.is_some(), "Details stays beside the fix");
    let _ = std::fs::remove_dir_all(&dir);
}
