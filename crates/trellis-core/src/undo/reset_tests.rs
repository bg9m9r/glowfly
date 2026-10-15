//! Hard resets stash uncommitted changes first; undoing one brings back HEAD and the changes,
//! staged and unstaged, and redoing it stashes again.

use super::*;
use crate::gitcli::run;
use crate::testutil::*;
use std::path::Path;

fn head_oid(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn run_all(dir: &Path, cmds: &[Vec<String>]) {
    for c in cmds {
        let (ok, out) = run(dir, c);
        assert!(ok, "git {c:?} failed: {out}");
    }
}

/// Runs a hard reset to `rev` the way Trellis does and returns what it recorded.
fn hard_reset(dir: &Path, rev: &str) -> Option<Action> {
    let kind = UndoKind::Reset(ResetMode::Hard);
    let b = before(dir, &kind).unwrap();
    run_all(dir, &hard_reset_commands(rev));
    after(dir, kind, b).unwrap()
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

fn repo(name: &str) -> (std::path::PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "c1"]);
    let c1 = head_oid(&dir);
    write(&dir, "a.txt", "one\ntwo\n");
    git(&dir, &["commit", "-q", "-am", "c2"]);
    let c2 = head_oid(&dir);
    (dir, c1, c2)
}

#[test]
fn undo_of_a_hard_reset_restores_uncommitted_changes() {
    let (dir, c1, c2) = repo("rst-dirty");
    write(&dir, "a.txt", "one\ntwo\nstaged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "a.txt", "one\ntwo\nstaged\nunstaged\n");
    write(&dir, "b.txt", "b edited\n");

    let action = hard_reset(&dir, &c1.to_string()).expect("recorded");
    let Action::Reset { stash: Some(_), from, to, .. } = action else { panic!("{action:?}") };
    assert_eq!((from, to, head_oid(&dir)), (c2, c1, c1));
    assert_eq!(read(&dir, "a.txt"), "one\n");

    match plan(&dir, &action, false).unwrap() {
        Plan::Run(cmds) => run_all(&dir, &cmds),
        other => panic!("{other:?}"),
    }
    assert_eq!(head_oid(&dir), c2);
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\nstaged\nunstaged\n");
    assert_eq!(read(&dir, "b.txt"), "b edited\n");
    let (_, staged, _) = crate::repo::status_at(&dir, true).unwrap();
    assert!(staged.iter().any(|f| f.path == "a.txt"), "the staged change is staged again");

    // Redo stashes the changes again rather than refusing or losing them.
    match plan(&dir, &action, true).unwrap() {
        Plan::Run(cmds) => run_all(&dir, &cmds),
        other => panic!("{other:?}"),
    }
    assert_eq!((head_oid(&dir), read(&dir, "a.txt")), (c1, "one\n".to_owned()));
    let again = redone(&dir, action).unwrap();
    assert!(matches!(again, Action::Reset { stash: Some(_), .. }), "{again:?}");
    match plan(&dir, &again, false).unwrap() {
        Plan::Run(cmds) => run_all(&dir, &cmds),
        other => panic!("{other:?}"),
    }
    assert_eq!(read(&dir, "b.txt"), "b edited\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hard_reset_to_head_is_undoable_when_it_discarded_changes() {
    let (dir, _, c2) = repo("rst-head");
    assert_eq!(hard_reset(&dir, "HEAD"), None, "a clean reset to HEAD changes nothing");
    write(&dir, "b.txt", "precious\n");
    let action = hard_reset(&dir, "HEAD").expect("recorded");
    assert_eq!(read(&dir, "b.txt"), "b\n");
    match plan(&dir, &action, false).unwrap() {
        Plan::Run(cmds) => run_all(&dir, &cmds),
        other => panic!("{other:?}"),
    }
    assert_eq!((head_oid(&dir), read(&dir, "b.txt")), (c2, "precious\n".to_owned()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dropped_stash_makes_the_undo_stale() {
    let (dir, c1, _) = repo("rst-dropped");
    write(&dir, "b.txt", "precious\n");
    let action = hard_reset(&dir, &c1.to_string()).expect("recorded");
    git(&dir, &["stash", "drop"]);
    let Plan::Refuse(r) = plan(&dir, &action, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale(), "{r:?}");
    assert_eq!(head_oid(&dir), c1, "nothing ran");
    let _ = std::fs::remove_dir_all(&dir);
}
