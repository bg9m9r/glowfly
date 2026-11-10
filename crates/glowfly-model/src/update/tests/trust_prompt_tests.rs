//! The open trust prompt without the runner (update/trust.rs): Show all and Show less on a
//! step row.

use super::tests::state;
use super::update;
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::TrustPrompt;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;

fn open_step(st: &AppState) -> Option<usize> {
    match &st.modal {
        Some(Modal::Trust(p)) => p.open_step,
        _ => panic!("the prompt stays open"),
    }
}

#[test]
fn show_all_opens_one_step_and_show_less_closes_it() {
    let mut st = state();
    let folder = PathBuf::from("C:/work/tools");
    st.modal = Some(Modal::Trust(TrustPrompt {
        name: "tools".into(),
        workdir: folder.clone(),
        real: folder,
        scan: Default::default(),
        refused: false,
        held: None,
        gained: None,
        terminal: false,
        incoming: None,
        unchecked: None,
        open_step: None,
    }));
    let toggle = |st: &mut AppState, i| assert!(update(st, Msg::Trust(TrustMsg::ToggleStep(i)), Instant::now()).is_empty());
    toggle(&mut st, 2);
    assert_eq!(open_step(&st), Some(2));
    toggle(&mut st, 0);
    assert_eq!(open_step(&st), Some(0), "one at a time");
    toggle(&mut st, 0);
    assert_eq!(open_step(&st), None);
    // Without the prompt it does nothing.
    st.modal = None;
    toggle(&mut st, 1);
    assert!(st.modal.is_none());
}
