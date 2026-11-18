//! Taking the trivial picks of a mixed merge through the runner: the files with only trivial
//! conflicts are resolved in one op and one undo entry, the toast's Undo brings every conflict
//! back exactly, Redo resolves them again; and the Preferences switch off opens a real conflict
//! without automatic picks.

use super::conflict_mark_undo_tests::{opened, stages};
use super::tests::{drive, scratch_repo};
use glowfly_core::testkit::trivial::mixed_trivial_merge;
use glowfly_core::undo::{Action, ConflictChange};
use glowfly_model::msg::{ConflictMsg, JobMsg, Msg};
use glowfly_model::state::AppState;
use std::path::{Path, PathBuf};

const FILES: [&str; 4] = ["tidy.txt", "also.txt", "mixed.txt", "logo.bin"];

fn mixed(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    mixed_trivial_merge(&dir);
    dir
}

fn conflicted(st: &AppState) -> Vec<String> {
    let mut v: Vec<String> = st.snap.as_ref().unwrap().unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.clone()).collect();
    v.sort();
    v
}

fn state_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    FILES.iter().map(|f| (stages(dir, f), std::fs::read(dir.join(f)).unwrap())).collect()
}

#[test]
fn the_trivial_picks_of_a_mixed_merge_are_taken_undone_and_redone() {
    let dir = mixed("trivial-flow");
    let before = state_of(&dir);
    let mut st = opened(&dir);
    assert_eq!(conflicted(&st), ["also.txt", "logo.bin", "mixed.txt", "tidy.txt"]);

    drive(&mut st, Msg::Conflict(ConflictMsg::TakeTrivialPicks));
    assert_eq!(conflicted(&st), ["logo.bin", "mixed.txt"], "tidy.txt and also.txt are resolved");
    let resolved: Vec<&str> = st.session.conflict_files.resolved.iter().map(|r| r.entry.path.as_str()).collect();
    assert!(resolved.contains(&"tidy.txt") && resolved.contains(&"also.txt"), "{resolved:?}");
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok);
    assert_eq!(
        toast.text,
        "Resolved 2 files with their trivial picks. 2 files still need a look. The activity log names the files that were skipped."
    );
    assert!(
        matches!(st.session.undo.undo.last(), Some(Action::Conflicts { change: ConflictChange::TrivialPicks, files, .. }) if files.len() == 2)
    );
    let resolved_state = state_of(&dir);

    let id = toast.id;
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Undo"));
    drive(&mut st, Msg::ToastAction(id));
    assert_eq!(conflicted(&st), ["also.txt", "logo.bin", "mixed.txt", "tidy.txt"]);
    assert_eq!(state_of(&dir), before, "stages and working bytes exactly as before");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(conflicted(&st), ["logo.bin", "mixed.txt"]);
    assert_eq!(state_of(&dir), resolved_state);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_switch_off_loads_a_real_conflict_without_picks() {
    let dir = mixed("trivial-switch-flow");
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("mixed.txt".into())));
    let v = st.session.conflict.view.as_ref().expect("open");
    assert_eq!((v.chunk_count(), v.picked()), (2, 1), "the whitespace-only chunk is picked");

    let mut st = opened(&dir);
    st.settings.conflict_auto_picks = false;
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("mixed.txt".into())));
    let v = st.session.conflict.view.as_ref().expect("open");
    assert!(v.auto.iter().all(Option::is_none));
    assert_eq!((v.chunk_count(), v.picked()), (2, 0));
    let _ = std::fs::remove_dir_all(&dir);
}
