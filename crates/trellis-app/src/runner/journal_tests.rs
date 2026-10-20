use crate::effect::{Effect, UndoOpts};
use crate::msg::{JobMsg, Msg};
use crate::runner::execute;
use git2::Oid;
use std::path::{Path, PathBuf};
use trellis_core::gitcli::{args, run};
use trellis_core::journal::{Op, notes};
use trellis_core::undo::{Action, ResetMode, UndoKind};

fn git(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?}: {out}");
    out
}

fn head(dir: &Path) -> Oid {
    Oid::from_str(git(dir, &["rev-parse", "HEAD"]).trim()).unwrap()
}

/// A repository with two commits on main.
fn repo(name: &str) -> (PathBuf, Oid, Oid) {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-app-test-journal-runner-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "T"), ("user.email", "t@e"), ("commit.gpgsign", "false")] {
        git(&dir, &["config", k, v]);
    }
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "one"]);
    let one = head(&dir);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "two"]);
    let two = head(&dir);
    (dir, one, two)
}

fn run_git(dir: &Path, cmd: &[&str], record: Option<UndoKind>, journal: Option<Op>) -> Option<Msg> {
    execute(Effect::RunGit {
        cwd: dir.to_path_buf(),
        cmds: vec![args(cmd)],
        write_file: None,
        record,
        journal,
        prompts: false,
        expect: None,
    })
}

#[test]
fn a_run_that_moves_head_writes_one_note_with_the_old_tip() {
    let (dir, one, two) = repo("moves");
    let one_s = one.to_string();
    let op = Op::Reset { mode: "hard".into(), to: one_s[..7].into() };
    let msg = run_git(&dir, &["reset", "--hard", &one_s], Some(UndoKind::Reset(ResetMode::Hard)), Some(op.clone()));
    assert!(matches!(msg, Some(Msg::Job(JobMsg::JobDone { ok: true, .. }))));
    let notes = notes::read(&dir);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].op, op);
    assert_eq!(notes[0].refs.len(), 1);
    assert_eq!(notes[0].refs[0].branch.as_deref(), Some("main"));
    assert_eq!(notes[0].refs[0].oid(), Some(two), "the note holds the state before the reset");
    assert!(notes[0].at > 1_600_000_000, "the runner's clock: {}", notes[0].at);
}

#[test]
fn a_failed_run_that_moved_nothing_writes_none() {
    let (dir, _, _) = repo("refused");
    let op = Op::Reset { mode: "hard".into(), to: "nosuch".into() };
    let msg = run_git(&dir, &["reset", "--hard", "nosuch"], Some(UndoKind::Reset(ResetMode::Hard)), Some(op));
    assert!(matches!(msg, Some(Msg::Job(JobMsg::JobDone { ok: false, .. }))));
    assert!(notes::read(&dir).is_empty());
}

#[test]
fn a_run_without_an_op_writes_none() {
    let (dir, one, _) = repo("no-op");
    run_git(&dir, &["reset", "--hard", &one.to_string()], None, None);
    assert_eq!(head(&dir), one);
    assert!(notes::read(&dir).is_empty());
}

#[test]
fn an_undo_that_moves_head_writes_an_undo_note() {
    let (dir, one, two) = repo("undo");
    git(&dir, &["reset", "--hard", &one.to_string()]);
    let action = Action::Reset { mode: ResetMode::Hard, branch: Some("main".into()), from: two, to: one, stash: None };
    let what = action.describe();
    let msg = execute(Effect::UndoRedo { cwd: dir.clone(), action, redo: false, opts: UndoOpts::default() });
    assert!(matches!(msg, Some(Msg::Job(JobMsg::JobDone { ok: true, .. }))));
    assert_eq!(head(&dir), two, "the undo put main back");
    let notes = notes::read(&dir);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].op, Op::Undo { what });
    assert_eq!(notes[0].refs[0].oid(), Some(one), "the state before the undo");
}
