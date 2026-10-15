//! Apply, pop and drop of a stash picked from a snapshot: if another tab, worktree or terminal
//! has since pushed or dropped a stash, `stash@{N}` is a different entry and nothing runs.

use super::palette_stash_tests::two_stashes;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::state::palette::StashAction;
use std::time::Instant;
use trellis_core::gitcli::args;

#[test]
fn the_job_names_the_position_and_expects_the_commit() {
    let (mut st, dir) = two_stashes("stash-job");
    let oid = st.snap.as_ref().unwrap().stashes[1].oid;
    let fx = update(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 1, oid }), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, expect, .. }] => {
            assert_eq!(cmds[..], [args(&["stash", "pop", "stash@{1}"])]);
            assert_eq!(expect, &Some(("stash@{1}".to_owned(), oid)));
        }
        other => panic!("expected one RunGit, got {} effects", other.len()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shifted_stash_list_refuses_the_drop() {
    let (mut st, dir) = two_stashes("stash-shifted");
    let second = st.snap.as_ref().unwrap().stashes[0].clone();
    // A terminal pushes a stash after the snapshot: stash@{0} is now that one.
    std::fs::write(dir.join("d.txt"), "d\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "third"]);
    for action in [StashAction::Drop, StashAction::Pop, StashAction::Apply] {
        drive(&mut st, Msg::Job(JobMsg::Stash { action, index: second.index, oid: second.oid }));
        let last = st.log.last().unwrap();
        assert!(!last.ok && last.text.contains("stash list changed") && last.text.contains("nothing was done"), "{}", last.text);
    }
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 3, "nothing dropped");
    assert!(snap.stashes[0].message.contains("third"));
    assert!(!snap.unstaged.iter().any(|f| f.path == "c.txt"), "nothing applied");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drop_logs_the_commit_so_it_can_be_recovered() {
    let (mut st, dir) = two_stashes("stash-dropped");
    let first = st.snap.as_ref().unwrap().stashes[1].clone();
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: first.index, oid: first.oid }));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.stashes[0].message.contains("second"));
    let log = st.log.iter().rev().find(|l| l.text.starts_with("Drop stash")).map(|l| l.text.clone()).unwrap();
    assert!(log.contains("Dropped") && log.contains(&first.oid.to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}
