//! Reloads of large diffs that resume the in-context parse from the previous highlight's
//! checkpoints: staging a hunk shows the rest colored at once, with no background pass.

use super::diff_tests::drive_holding_diffs;
use super::highlight_tests::{drive_holding_highlights, land, shown};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, StageMsg};
use crate::runner;
use glowfly_core::highlight;
use glowfly_core::repo::{DiffTarget, HunkOp};
use std::time::Instant;

/// `fn main() {`, 3,000 `let` lines, `}`; lines whose number is in `changed` get `+ 1`.
fn body(changed: &[usize]) -> String {
    let lets: String = (1..=3000)
        .map(|n| if changed.contains(&n) { format!("    let v{n} = {n} + 1;\n") } else { format!("    let v{n} = {n};\n") })
        .collect();
    format!("fn main() {{\n{lets}}}\n")
}

#[test]
fn staging_a_hunk_of_a_big_file_finishes_inline() {
    let dir = scratch_repo("hl-memo-stage");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("big.rs"), body(&[])).unwrap();
    git(&dir, &["add", "big.rs"]);
    git(&dir, &["commit", "-q", "-m", "big"]);
    std::fs::write(dir.join("big.rs"), body(&[5, 2_900])).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let target = DiffTarget::Unstaged("big.rs".into());
    let mut held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(target)));
    let d = shown(&st);
    assert_eq!(d.hunks.len(), 2);
    assert!(d.highlight.as_ref().is_some_and(|h| h.is_pending()), "over {} lines in context", highlight::MAX_LINES);
    assert_eq!(held.len(), 1);
    let msg = runner::testing::execute(held.remove(0)).expect("finished");
    update(&mut st, msg, Instant::now());
    assert!(shown(&st).highlight.as_ref().is_some_and(|h| !h.is_pending() && h.checkpoints() > 0));

    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    let stage = Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "big.rs".into(), index: 0, generation });
    let loads = drive_holding_diffs(&mut st, stage);
    assert!(!loads.is_empty(), "the diff reloads");
    let fx = land(&mut st, loads);
    let d = shown(&st);
    assert_eq!(d.hunks.len(), 1);
    assert!(d.highlight.as_ref().is_some_and(|h| !h.is_pending()), "no provisional colours");
    assert!(!fx.iter().any(|e| matches!(e, Effect::HighlightDiff { .. })), "no background pass: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
