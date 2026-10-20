//! Branch colours through the app: worked out by the load, and kept when a status read turns
//! the work tree dirty and the graph is laid out again with its WIP row.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, WatchMsg};
use crate::runner;
use std::time::Instant;
use trellis_core::branch_colour::Swatch;

#[test]
fn colours_survive_the_clean_to_dirty_relayout() {
    // main, with topic and fix-1 (whose names share a slot) one commit each on top of it.
    let dir = scratch_repo("branch-colour");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    for b in ["topic", "fix-1"] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        std::fs::write(dir.join(format!("{b}.txt")), b).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", b]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.wip.is_none(), "clean");
    let keys = ["main", "topic", "fix-1"];
    let before: Vec<Swatch> = keys.iter().map(|k| snap.branch_swatch(k)).collect();
    assert_eq!(before[0], Swatch::Main);
    assert_ne!(before[1], before[2], "side by side, never one hue");

    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    let workdir = st.workdir().unwrap();
    let changed =
        Msg::Scoped { workdir, msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() })) };
    let fx = update(&mut st, changed, Instant::now());
    let status = fx.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).and_then(runner::execute).expect("a status read");
    let _ = update(&mut st, status, Instant::now());

    let snap = st.snap.as_ref().unwrap();
    assert!(snap.graph.wip.is_some(), "the WIP row appears");
    let after: Vec<Swatch> = keys.iter().map(|k| snap.branch_swatch(k)).collect();
    assert_eq!(after, before);
    let wip = snap.graph.owners.wip.expect("the WIP row has an owner");
    assert_eq!(snap.graph.owners.key(wip), Some("main"), "the WIP row takes HEAD's branch");
    let _ = std::fs::remove_dir_all(&dir);
}
