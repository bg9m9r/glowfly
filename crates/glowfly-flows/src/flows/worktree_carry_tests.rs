//! Other worktrees' rows through loads, end to end: opening and F5 show them Pending until the
//! count right after the load fills them in, a full reload carries the counts of a worktree
//! whose folder and HEAD are unchanged (its row stays and nothing says "counting") until the
//! fresh count replaces them, and one whose HEAD moved starts over as Pending.

use super::tests::{git, scratch_repo, state};
use super::worktree_counts_tests::{SIDE, counts, linked_info, opened, run_count, step};
use glowfly_core::repo::{ChangeCounts, ChangesRead};
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::worktrees::right_text;
use std::path::Path;
use std::time::Instant;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The (worktree, row it sits before) of each WIP row of the other worktrees in the graph.
fn rows(st: &AppState) -> Vec<(usize, usize)> {
    st.snap.as_ref().unwrap().graph.worktree_wips.iter().map(|w| (w.worktree, w.before)).collect()
}

#[test]
fn opening_shows_other_worktrees_pending_until_counted() {
    let dir = scratch_repo("wtcarry-open");
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    std::fs::write(linked.join("a.txt"), "changed there\n").unwrap();
    std::fs::write(linked.join("b.txt"), "new there\n").unwrap();
    let mut st = state();
    let fx = step(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Pending, ChangeCounts::default()));
    assert!(right_text(wt).ends_with(" · counting…"), "{}", right_text(wt));
    assert!(rows(&st).is_empty(), "no WIP row before the count");
    assert_eq!(counts(&fx).len(), 1, "{fx:?}");

    run_count(&mut st, fx, Instant::now());
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Counted, SIDE));
    assert_eq!(rows(&st).iter().map(|r| r.0).collect::<Vec<_>>(), [1]);
    cleanup(&[&linked, &dir]);
}

#[test]
fn a_full_reload_keeps_the_rows_until_the_count_lands() {
    let (mut st, dir, linked) = opened("wtcarry-reload");
    let before = rows(&st);
    assert_eq!(before.len(), 1);
    std::fs::write(linked.join("c.txt"), "more there\n").unwrap();
    git(&dir, &["branch", "extra"]);

    let fx = step(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Carried, SIDE), "the last counts, until the new ones land");
    assert_eq!(rows(&st), before, "its row is where it was");
    assert!(!right_text(wt).contains("counting"), "{}", right_text(wt));
    assert_eq!(counts(&fx).len(), 1, "{fx:?}");

    run_count(&mut st, fx, Instant::now());
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Counted, ChangeCounts { added: 2, ..SIDE }));
    assert_eq!(rows(&st), before);
    cleanup(&[&linked, &dir]);
}

#[test]
fn a_worktree_whose_head_moved_is_pending_after_the_reload() {
    let (mut st, dir, linked) = opened("wtcarry-head");
    git(&linked, &["add", "a.txt"]);
    git(&linked, &["commit", "-q", "-m", "side work"]);

    let fx = step(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Pending, ChangeCounts::default()));
    assert!(rows(&st).is_empty(), "no old counts above the new commit");
    assert_eq!(counts(&fx).len(), 1, "{fx:?}");

    run_count(&mut st, fx, Instant::now());
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Counted, ChangeCounts { modified: 0, added: 1, deleted: 0 }));
    assert_eq!(rows(&st).len(), 1);
    cleanup(&[&linked, &dir]);
}

#[test]
fn f5_reads_fresh() {
    let (mut st, dir, linked) = opened("wtcarry-f5");
    let fx = step(&mut st, Msg::Repo(RepoMsg::Refresh));
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Pending, ChangeCounts::default()), "F5 carries nothing");
    assert_eq!(counts(&fx).len(), 1, "{fx:?}");
    run_count(&mut st, fx, Instant::now());
    assert_eq!((linked_info(&st).read, linked_info(&st).changes), (ChangesRead::Counted, SIDE));
    cleanup(&[&linked, &dir]);
}
