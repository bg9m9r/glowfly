//! Pop and apply with `--index`: recorded and undone like the plain form, and redone with the
//! staged state (`Plan::ApplyStash`).

use super::stash_tests::{base, clean, record_kind, split, stashes};
use super::tests::run_plan;
use super::*;
use crate::stash_ops::{self, StashBack, apply_command};
use crate::stash_ops::{apply_tests::mixed, auto_put_back_tests::index_wont_fit};
use crate::testutil::{git, write};

#[test]
fn pop_index_is_recorded_on_a_clean_tree_only() {
    let (dir, _) = base("undo-stash-index-record");
    write(&dir, "a.txt", "a edited\n");
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    let (lost, action) = record_kind(&dir, &["stash", "pop", "--index", "stash@{0}"]);
    assert!(!lost);
    assert!(matches!(action, Some(Action::StashApply { pop: true, .. })), "{action:?}");
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, _) = base("undo-stash-index-dirty");
    write(&dir, "a.txt", "a edited\n");
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    write(&dir, "b.txt", "local\n");
    let (lost, action) = record_kind(&dir, &["stash", "pop", "--index", "stash@{0}"]);
    assert!(lost && action.is_none(), "{lost} {action:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_clears_the_index_and_redo_stages_it_again() {
    for pop in [true, false] {
        let verb = if pop { "pop" } else { "apply" };
        let (dir, oid) = mixed("undo-stash-index-redo");
        let (lost, action) = record_kind(&dir, &["stash", verb, "--index", "stash@{0}"]);
        assert!(!lost);
        let mut action = action.expect("recorded");
        let applied = split(&dir);
        assert!(!applied.1.is_empty(), "something is staged: {applied:?}");

        for _ in 0..2 {
            run_plan(&dir, plan(&dir, &action, false).unwrap());
            assert!(clean(&dir), "{:?}", split(&dir));
            assert_eq!(stashes(&dir).len(), 1);
            assert_eq!(stashes(&dir)[0].1, oid);

            let redo = plan(&dir, &action, true).unwrap();
            assert_eq!(redo, Plan::ApplyStash { pop, index: 0, stash: oid });
            run_plan(&dir, redo);
            assert_eq!(split(&dir), applied);
            assert_eq!(stashes(&dir).len(), usize::from(!pop));
            action = redone(&dir, action).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn redo_of_an_apply_that_fell_back_falls_back_again() {
    let (dir, oid) = index_wont_fit("undo-stash-index-fallback", &[(4, "four")]);
    let kind = UndoKind::classify(&apply_command(true, 0, true)).expect("undoable");
    let b = before(&dir, &kind).unwrap();
    let (back, output) = stash_ops::apply(&dir, true, 0, oid);
    assert_eq!(back, StashBack::Unstaged, "{output}");
    let action = after(&dir, kind, b).unwrap().expect("recorded");
    let unstaged = split(&dir);
    assert!(unstaged.1.is_empty(), "{unstaged:?}");

    run_plan(&dir, plan(&dir, &action, false).unwrap());
    assert!(clean(&dir), "{:?}", split(&dir));
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, Plan::ApplyStash { pop: true, index: 0, stash: oid });
    run_plan(&dir, redo);
    assert_eq!(split(&dir), unstaged);
    assert!(stashes(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn push_undo_over_unrelated_staged_changes_pops_plainly() {
    let (dir, _) = base("undo-stash-push-staged");
    write(&dir, "a.txt", "a staged\n");
    git(&dir, &["add", "a.txt"]);
    let (_, action) = record_kind(&dir, &["stash", "push", "-u"]);
    let action = action.expect("recorded");
    assert!(clean(&dir), "{:?}", split(&dir));
    write(&dir, "b.txt", "b staged\n");
    git(&dir, &["add", "b.txt"]);
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, super::tests::cmds(&[&["stash", "pop", "stash@{0}"]]));
    run_plan(&dir, undo);
    assert_eq!(split(&dir), (vec!["Ma.txt".to_owned()], vec!["Mb.txt".to_owned()]));
    assert!(stashes(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
