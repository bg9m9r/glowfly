//! Headless tests for the WIP panel's operation steps: Show and Hide open and fold a step
//! (view state only, nothing saved), and the choice gives way once the conflicts are resolved.

use super::tests::state;
use super::update;
use crate::msg::{LayoutMsg, Msg};
use crate::state::op_fixture::{rows, snapshot};
use crate::state::op_steps::OpStep;
use git2::RepositoryState;
use std::time::Instant;

#[test]
fn show_and_hide_fold_a_step_until_the_operation_moves_on() {
    let mut st = state();
    let mut snap = snapshot("update-op-steps");
    snap.state = RepositoryState::Merge;
    snap.unstaged = rows(&[("a.txt", '!')]);
    st.snap = Some(snap);
    let open = |st: &crate::state::AppState, step| st.view.op_steps.is_open(st.snap.as_ref().unwrap(), step);
    assert!(!open(&st, OpStep::Finish), "Commit the merge waits for the conflicts");

    let fx = update(&mut st, Msg::Layout(LayoutMsg::ToggleOpStep(OpStep::Finish)), Instant::now());
    assert!(fx.is_empty(), "view state only");
    assert!(open(&st, OpStep::Finish));
    update(&mut st, Msg::Layout(LayoutMsg::ToggleOpStep(OpStep::Resolve)), Instant::now());
    assert!(!open(&st, OpStep::Resolve));

    // The conflict resolved: step 3 is the one now, open; step 1 folds to its line.
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M')]);
    assert!(open(&st, OpStep::Finish));
    assert!(!open(&st, OpStep::Resolve));
    assert!(!open(&st, OpStep::Check));
}
