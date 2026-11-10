//! A stopped rebase Glowfly started, as the trust scan reads it (`trust::scan_at`): the `exec`
//! lines Glowfly adds around a reworded commit aren't steps to ask about while they are exactly
//! as written, an `exec` step the user added is, and so is one of Glowfly's lines once edited.

use super::plan_tests::linear;
use super::run_tests::{log, request_steps, start_copy, state};
use super::*;
use crate::settings::SavedTrust;
use crate::trust::{allows, scan_at};
use std::path::{Path, PathBuf};

/// Starts a rebase of c2..c4 that stops to edit c2 and rewords c4 after `more` changed its
/// steps; returns the repository, stopped, and the decision a scan before the rebase gives.
fn stopped(name: &str, more: impl FnOnce(&mut Vec<TodoStep>)) -> (PathBuf, SavedTrust) {
    let (dir, [_, c2, ..]) = linear(name);
    let before = SavedTrust::trusted(Some(scan_at(&dir).unwrap().print()));
    let req = request_steps(&dir, RebaseBase::FromCommit(c2), |steps| {
        steps[0].commit_mut().unwrap().action = RebaseAction::Edit;
        let c4 = steps[2].commit_mut().unwrap();
        c4.action = RebaseAction::Reword;
        c4.new_message = Some("c4, reworded".into());
        more(steps);
    });
    assert!(matches!(start_copy(&dir, &req), Outcome::Stopped { ok: true, .. }));
    (dir, before)
}

fn todo_path(dir: &Path) -> PathBuf {
    dir.join(".git/rebase-merge/git-rebase-todo")
}

#[test]
fn glowflys_own_lines_in_a_stopped_rebase_are_no_reason_to_ask() {
    let (dir, before) = stopped("rebase-own-lines", |_| {});
    let todo = std::fs::read_to_string(todo_path(&dir)).unwrap();
    assert_eq!(todo.lines().filter(|l| l.starts_with("exec ")).count(), 2, "{todo}");
    let s = scan_at(&dir).unwrap();
    assert!(s.steps.is_empty() && s.is_empty(), "{s:?}");
    assert!(allows(None, &s), "a repository nothing was decided for still runs nothing");
    assert!(allows(Some(&before), &s), "the decision made before the rebase still covers it");
    assert!(matches!(resume(&dir, Resume::Continue).unwrap(), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4, reworded", "c3", "c2", "c1"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_exec_step_the_user_added_is_listed_beside_glowflys_own() {
    let (dir, before) = stopped("rebase-own-lines-user", |steps| steps.push(TodoStep::Exec("git status".into())));
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.steps, ["exec git status"]);
    assert!(!allows(None, &s) && !allows(Some(&before), &s));
    // Trusted with that step listed, it continues.
    assert!(allows(Some(&SavedTrust::trusted(Some(s.print()))), &scan_at(&dir).unwrap()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_own_line_that_was_edited_is_listed() {
    let (dir, before) = stopped("rebase-own-lines-edited", |_| {});
    let todo = std::fs::read_to_string(todo_path(&dir)).unwrap();
    let base = todo.lines().find(|l| l.starts_with("exec git rev-parse HEAD >")).expect("the line that records HEAD");
    // Git may write the keyword short (rebase.abbreviateCommands): still Glowfly's line.
    std::fs::write(todo_path(&dir), todo.replace(base, &base.replacen("exec ", "x ", 1))).unwrap();
    assert!(scan_at(&dir).unwrap().is_empty());
    let edited = format!("{base}; git status");
    std::fs::write(todo_path(&dir), todo.replace(base, &edited)).unwrap();
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.steps, [edited]);
    assert!(!allows(Some(&before), &s));
    // The same line for another folder's message files isn't this repository's.
    let elsewhere = base.replace("/glowfly-rebase/", "/elsewhere/glowfly-rebase/");
    std::fs::write(todo_path(&dir), todo.replace(base, &elsewhere)).unwrap();
    assert_eq!(scan_at(&dir).unwrap().steps, [elsewhere]);
    // Any character more is another command to sh: a no-break space or a form feed after the
    // line or before its command, which git doesn't drop as it drops spaces and tabs.
    for other in ["\u{a0}", "\x0c", "\u{3000}", "\x0b"] {
        let after = format!("{base}{other}");
        let before_command = base.replacen("exec ", &format!("exec {other}"), 1);
        let between = base.replacen("exec ", &format!("exec  {other} "), 1);
        for line in [after, before_command, between] {
            std::fs::write(todo_path(&dir), todo.replace(base, &line)).unwrap();
            let s = scan_at(&dir).unwrap();
            assert_eq!(s.steps.len(), 1, "{line:?}");
            assert!(s.steps[0].contains(other) && !allows(Some(&before), &s), "{line:?}");
        }
    }
    // Spaces and tabs around the keyword are git's to drop: still Glowfly's line.
    std::fs::write(todo_path(&dir), todo.replace(base, &format!(" \t{}", base.replacen("exec ", "exec \t ", 1)))).unwrap();
    assert!(scan_at(&dir).unwrap().is_empty());
    assert_eq!(state(&dir), git2::RepositoryState::RebaseInteractive);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn an_own_line_whose_file_is_a_link_is_listed() {
    let (dir, _) = stopped("rebase-own-lines-link", |_| {});
    let msg_dir = dir.join(".git/glowfly-rebase");
    let record =
        std::fs::read_dir(&msg_dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "txt")).unwrap();
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(dir.join("1.txt"), &record).unwrap();
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.steps.len(), 1, "the amend reads its message through a link: {s:?}");
    assert!(s.steps[0].starts_with("exec h=$(git rev-parse"));
    let _ = std::fs::remove_dir_all(&dir);
}
