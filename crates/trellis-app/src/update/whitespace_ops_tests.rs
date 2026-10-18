//! Headless tests for hunk and line operations on a working-tree diff shown with whitespace
//! changes hidden: they carry the setting to core, which stages, unstages or discards only the
//! changes shown, and a discard can still be undone exactly.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::msg::{DiffMsg, JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::lines::LineClick::Only;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, HunkOp};
use trellis_core::undo::{Action, Discarded};

const OLD: &str = "a\n    x\n    y\nb\n";
/// `OLD` with `x` and `y` wrapped in a block, re-indented.
const NEW: &str = "a\nif {\n        x\n        y\n}\nb\n";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// The scratch repository with `f.txt` committed as `OLD` and changed to `NEW`, open, with
/// whitespace hidden and `target` on screen.
fn reindented(name: &str, staged: bool) -> (AppState, PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("f.txt"), OLD).unwrap();
    git(&dir, &["add", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "f"]);
    std::fs::write(dir.join("f.txt"), NEW).unwrap();
    let target = if staged {
        git(&dir, &["add", "f.txt"]);
        DiffTarget::Staged("f.txt".into())
    } else {
        DiffTarget::Unstaged("f.txt".into())
    };
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert_eq!(st.diff.as_ref().map(|(t, d)| (t, d.hunks.len())), Some((&target, 1)));
    (st, dir, target)
}

fn changes(dir: &Path, target: &DiffTarget) -> Vec<String> {
    let d = repo::load_diff(dir, target).unwrap();
    d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| format!("{}{}", l.origin, l.text)).collect()
}

fn hunk(op: HunkOp, generation: u64) -> Msg {
    Msg::Stage(StageMsg::Hunk { op, path: "f.txt".into(), index: 0, generation })
}

fn read(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("f.txt")).unwrap()
}

const REINDENT: [&str; 4] = ["-    x", "-    y", "+        x", "+        y"];

#[test]
fn stage_hunk_applies_only_the_shown_change() {
    let (mut st, dir, _) = reindented("ws-ops-stage", false);
    let generation = st.hunk_ops_gen().expect("hunk buttons on");
    let fx = run(&mut st, hunk(HunkOp::Stage, generation));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Hunk { op: HunkOp::Stage, ignore_whitespace: true, .. }, .. }]), "{fx:?}");
    finish(&mut st, fx);
    assert!(st.log.iter().all(|l| l.ok), "{:?}", st.log.last().map(|l| &l.text));
    assert_eq!(changes(&dir, &DiffTarget::Staged("f.txt".into())), ["+if {", "+}"]);
    assert_eq!(changes(&dir, &DiffTarget::Unstaged("f.txt".into())), REINDENT, "the re-indent is left unstaged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lines_can_be_selected_and_staged() {
    let (mut st, dir, _) = reindented("ws-ops-lines", false);
    let close = st.diff.as_ref().unwrap().1.hunks[0].lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    assert!(run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: close, how: Only })).is_empty());
    assert!(!st.view.diff_lines.is_empty(), "selection works with whitespace hidden");
    let fx = run(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Stage)));
    assert!(
        matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Lines { lines, ignore_whitespace: true, .. }, .. }] if *lines == [close]),
        "{fx:?}"
    );
    finish(&mut st, fx);
    assert_eq!(changes(&dir, &DiffTarget::Staged("f.txt".into())), ["+}"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_hunk_keeps_the_reindent_and_undoes_exactly() {
    let (mut st, dir, _) = reindented("ws-ops-discard", false);
    let generation = st.hunk_ops_gen().unwrap();
    run(&mut st, Msg::confirm("Discard hunk", "Discard?".into(), "Discard", hunk(HunkOp::Discard, generation)));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(read(&dir), "a\n        x\n        y\nb\n");
    assert!(
        matches!(st.undo.undo.last(), Some(Action::Discard { what: Discarded::Hunk, path, .. }) if path == "f.txt"),
        "{:?}",
        st.undo.undo
    );
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), NEW, "the working copy is back byte for byte");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_hunk_on_a_staged_diff() {
    let (mut st, dir, _) = reindented("ws-ops-unstage", true);
    let generation = st.hunk_ops_gen().unwrap();
    let fx = run(&mut st, hunk(HunkOp::Unstage, generation));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Hunk { op: HunkOp::Unstage, ignore_whitespace: true, .. }, .. }]), "{fx:?}");
    finish(&mut st, fx);
    assert_eq!(changes(&dir, &DiffTarget::Staged("f.txt".into())), REINDENT, "the re-indent stays staged");
    assert_eq!(read(&dir), NEW);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_whitespace_shown_the_flag_is_off() {
    let (mut st, dir, _) = reindented("ws-ops-shown", false);
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(false)));
    let generation = st.hunk_ops_gen().unwrap();
    let fx = run(&mut st, hunk(HunkOp::Stage, generation));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Hunk { ignore_whitespace: false, .. }, .. }]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
