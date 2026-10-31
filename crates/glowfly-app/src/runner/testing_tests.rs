//! The sandbox's own checks: whatever a test runs through `runner::testing` (one effect, or the
//! whole-app tests' runner) uses the stand-ins, never the real trash or sequence editor.

use super::*;
use crate::effect::DeleteMode;
use glowfly_core::repo::{Trasher, Untrasher};
use std::ptr::fn_addr_eq;

/// Whether `s` is the tests' stand-ins: the folder trash for all three, and `cp` as the editor.
fn stand_ins(s: &Seams) -> bool {
    let (t, real) = (s.trashers, Seams::REAL.trashers);
    let trash: [Trasher; 2] = [t.system, t.home];
    let stand_in = trash.iter().all(|f| fn_addr_eq(*f, SEAMS.trashers.system as Trasher));
    let real_trash = trash.iter().any(|f| fn_addr_eq(*f, real.system) || fn_addr_eq(*f, real.home));
    let untrash = fn_addr_eq(t.restore, SEAMS.trashers.restore as Untrasher) && !fn_addr_eq(t.restore, real.restore);
    stand_in && !real_trash && untrash && matches!((s.sequence_editor)(), SequenceEditor::Copy)
}

#[test]
fn the_stand_ins_are_not_the_real_trash_or_editor() {
    assert!(stand_ins(&SEAMS));
    assert!(!stand_ins(&Seams::REAL), "the app's seams are the real ones");
    assert!(matches!((Seams::REAL.sequence_editor)(), SequenceEditor::Helper(_)), "the app installs todos itself");
    assert!(SEAMS.chunk > Seams::REAL.chunk);
}

#[test]
fn the_whole_app_tests_runner_uses_the_stand_ins() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let runner = sandboxed(tx, Waker::none(), glowfly_core::paths::temp_dir());
    assert!(stand_ins(&runner.seams));
    assert!(runner.sandbox.is_some());
    let (tx, _rx) = std::sync::mpsc::channel();
    assert!(!stand_ins(&Runner::new(tx, Waker::none()).seams), "the app's runner uses the real ones");
}

#[test]
fn a_delete_run_by_a_test_goes_to_the_stand_in_bin() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-testing-trash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    glowfly_core::testkit::git(&dir, &["init", "-q", "-b", "main"]);
    let contents = format!("only in the stand-in bin {}\n", std::process::id());
    std::fs::write(dir.join("gone.txt"), &contents).unwrap();
    let op = FileOp::Delete { path: "gone.txt".into(), mode: DeleteMode::Trash };
    let msg = execute(Effect::WorkFile { workdir: dir.clone(), op });
    assert!(msg.is_some());
    assert!(!dir.join("gone.txt").exists());
    let binned = std::fs::read_dir(test_bin()).unwrap().flatten().any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents));
    assert!(binned, "the file is in the stand-in bin");
    let _ = std::fs::remove_dir_all(&dir);
}
