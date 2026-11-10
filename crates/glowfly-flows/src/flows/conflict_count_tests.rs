//! A job that stopped on conflicts counts its files from git's CONFLICT lines, then from the
//! first complete status read after it: each conflicted path once (names that aren't UTF-8
//! included), with View conflicts on a path the status lists and git can be given, else on the
//! WIP list. A partial read keeps git's count, saying it may be incomplete, and keeps waiting; a
//! failed read and the next job end the wait.

use super::auto_stash_tests::up;
use super::failure_headline_tests::{last_toast, opened, running};
use super::tests::{finish, git, said};
use glowfly_core::gitcli::{args, run_with_input};
use glowfly_core::repo::{FileEntry, Hazard, PathsStatus};
use glowfly_model::effect::StatusRead;
use glowfly_model::logs::LogsMsg;
use glowfly_model::msg::{ConflictMsg, JobMsg, LoadMsg, Msg, RepoMsg};
use glowfly_model::state::{AppState, Selection};
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
fn merge_ended(st: &mut AppState, dir: &Path, line: &str) -> Vec<glowfly_model::effect::Effect> {
    running(st, dir, "Merge");
    let output = format!("$ git merge side\n{line}\nAutomatic merge failed; fix conflicts and then commit the result.");
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }))
}

/// What the conflicts toast's fix runs, through the run's `LogsMsg::Fix` proxy, if it has one.
fn view_msg(st: &AppState) -> Option<&Msg> {
    let t = st.toasts.iter().find(|t| t.text.starts_with("Stopped on conflicts"))?;
    let msg = &t.action.as_ref()?.msg;
    let msg = match **msg {
        Msg::Logs(LogsMsg::Fix(id)) => &st.logs.fix(id)?.msg,
        _ => msg,
    };
    Some(&**msg)
}

/// What the conflicts toast's fix opens, if it opens a file.
fn view_target(st: &AppState) -> Option<String> {
    match view_msg(st)? {
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
        result: Err(said("could not read the status")),
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

/// "at least <n> file(s)": git's count when a status read may have left conflicted files out.
const HEDGED_ONE: &str = "Stopped on conflicts in at least 1 file (git's list may be incomplete). Resolve them, then continue or abort.";

/// The merge's run summary.
fn summary(st: &AppState) -> &str {
    &st.log.iter().rev().find_map(|e| e.run.as_ref()).expect("the merge's run").summary
}

/// The watcher's read of r1.txt, as status_paths.rs gives it: no conflicted row, so the lists
/// keep the ones from before the job.
fn path_read(st: &mut AppState) {
    let ps =
        PathsStatus { covered: vec!["r1.txt".into()], unstaged: vec![], state: git2::RepositoryState::Merge, submodule_touched: false };
    status_loaded(st, StatusRead::Paths(ps));
}

fn status_loaded(st: &mut AppState, read: StatusRead) {
    let msg = LoadMsg::StatusLoaded {
        generation: st.load_gen,
        result: Ok(read),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    up(st, Msg::Load(msg));
}

#[test]
fn a_partial_read_keeps_gits_count_until_a_whole_one() {
    let (mut st, dir) = opened("cc-partial");
    let line = rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, &line);
    path_read(&mut st);
    assert_eq!(last_toast(&st), HEDGED_ONE);
    assert_eq!(summary(&st), HEDGED_ONE);
    assert!(st.logs.recount.as_ref().is_some_and(|r| r.partial), "still waiting");
    assert_eq!(view_target(&st).as_deref(), Some("b.txt"));
    path_read(&mut st);
    assert_eq!(last_toast(&st), HEDGED_ONE, "said once");
    finish(&mut st, fx);
    assert!(st.toasts.iter().any(|t| t.text == FOUR));
    assert_eq!(summary(&st), FOUR);
    assert!(st.logs.recount.is_none());
    assert_eq!(view_target(&st).as_deref(), Some("b.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_partial_then_failed_read_keeps_the_hedge() {
    let (mut st, dir) = opened("cc-partial-failed");
    let line = rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, &line);
    path_read(&mut st);
    let failed = LoadMsg::RepoUnchanged {
        generation: st.load_gen,
        result: Err(said("could not read the status")),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    up(&mut st, Msg::Load(failed));
    assert!(st.logs.recount.is_none(), "the failed read ended it");
    assert!(st.toasts.iter().any(|t| t.text == HEDGED_ONE));
    finish(&mut st, fx);
    assert!(!st.toasts.iter().any(|t| t.text == FOUR));
    assert_eq!(summary(&st), HEDGED_ONE);
    let _ = std::fs::remove_dir_all(&dir);
}

/// "café.txt" in Latin-1: a name that isn't UTF-8, and how its file row shows it.
const CAFE: &[u8] = b"caf\xe9.txt";
const CAFE_SHOWN: &str = "caf\\xE9.txt";

fn not_utf8_row() -> FileEntry {
    FileEntry { hazard: Some(Hazard::NotUtf8(CAFE.to_vec())), ..FileEntry::new(CAFE_SHOWN, '!') }
}

/// A merge whose CONFLICT lines named no file, then a complete status read listing `unstaged`.
fn unnamed_then_read(name: &str, unstaged: Vec<FileEntry>) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = opened(name);
    merge_ended(&mut st, &dir, "");
    assert_eq!(last_toast(&st), "Stopped on conflicts. Resolve them, then continue or abort.");
    status_loaded(&mut st, StatusRead::Whole(unstaged, vec![], git2::RepositoryState::Merge));
    assert!(st.logs.recount.is_none());
    (st, dir)
}

#[test]
fn a_name_that_is_not_utf8_is_counted_but_not_opened() {
    let (st, dir) = unnamed_then_read("cc-not-utf8-rows", vec![not_utf8_row(), FileEntry::new("d.txt", '!')]);
    let two = "Stopped on conflicts in 2 files. Resolve them, then continue or abort.";
    assert_eq!(last_toast(&st), two);
    assert_eq!(summary(&st), two);
    assert_eq!(view_target(&st).as_deref(), Some("d.txt"), "the first name git can be given");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_names_that_are_not_utf8_select_the_wip_list() {
    let (mut st, dir) = unnamed_then_read("cc-not-utf8-only", vec![not_utf8_row()]);
    assert_eq!(last_toast(&st), "Stopped on conflicts in 1 file. Resolve it, then continue or abort.");
    assert!(matches!(view_msg(&st), Some(Msg::Repo(RepoMsg::SelectWip))));
    let t = st.toasts.last().unwrap();
    assert!(t.details.is_some(), "Details stays beside View conflicts");
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("View conflicts"));
    let id = t.id;
    st.session.selection = Selection::None;
    let logged = st.log.len();
    up(&mut st, Msg::ToastAction(id));
    assert_eq!(st.session.selection, Selection::Wip);
    assert_eq!(st.log.len(), logged, "nothing refused");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A real merge conflict on a file whose name isn't UTF-8: git's line names it lossily, so
/// the status replaces View conflicts with the WIP list. Only where the file system takes it.
#[cfg(unix)]
#[test]
fn a_real_conflict_on_a_name_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let (mut st, dir) = opened("cc-not-utf8");
    let file = dir.join(std::ffi::OsStr::from_bytes(CAFE));
    if std::fs::write(&file, "base\n").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return eprintln!("skipped: this file system takes only UTF-8 names");
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(&file, "side\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "side"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(&file, "main\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "main"]);
    let (ok, out) = run_with_input(&dir, &args(&["merge", "side"]), "", &[]);
    assert!(!ok, "{out}");
    let line = out.lines().find(|l| l.starts_with("CONFLICT (content)")).unwrap_or_default().to_owned();
    let fx = merge_ended(&mut st, &dir, &line);
    finish(&mut st, fx);
    assert!(last_toast(&st).starts_with("Stopped on conflicts in 1 file."), "{}", last_toast(&st));
    assert!(matches!(view_msg(&st), Some(Msg::Repo(RepoMsg::SelectWip))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_toast_whose_lines_named_no_file_gets_view_conflicts() {
    let (mut st, dir) = opened("cc-unlisted");
    rename_conflicts(&dir, "b.txt");
    let fx = merge_ended(&mut st, &dir, "");
    assert_eq!(last_toast(&st), "Stopped on conflicts. Resolve them, then continue or abort.");
    assert_eq!(view_target(&st), None);
    finish(&mut st, fx);
    let t = st.toasts.iter().find(|t| t.text == FOUR).expect("the status's count");
    let action = t.action.as_ref().expect("View conflicts");
    assert!(matches!(*action.msg, Msg::Logs(LogsMsg::Fix(_))));
    assert_eq!(action.label, "View conflicts");
    assert!(t.details.is_some(), "Details stays beside the fix");
    assert_eq!(view_target(&st).as_deref(), Some("b.txt"), "the first unmerged path in list order");
    assert!(st.logs.recount.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
