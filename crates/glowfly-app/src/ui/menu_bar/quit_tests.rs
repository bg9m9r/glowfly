//! The quit question while a push runs, as the window draws it: the danger confirmation with
//! Cancel focused and Quit anyway on the right, naming the job and its tab (and, in the second,
//! a terminal running a program).

use crate::msg::Msg;
use crate::state::RunningJob;
use crate::state::quit_confirm::quit_confirm;
use crate::state::terminal::busy::BusyTerminal;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

#[test]
fn quit_confirm_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "quit-confirm");
    st.job = Some(RunningJob {
        label: "Push".into(),
        workdir: st.tabs[0].path.clone(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    });
    st.modal = quit_confirm(&st, &[]);
    assert!(st.modal.is_some());
    let mut h = harness(st, true);
    h.run_ok();
    h.check_snapshot("quit_confirm");
    assert!(h.get_by_label("Cancel").is_focused(), "the safe answer has the focus");
    h.get_by_label("Quit anyway").click();
    h.run_ok();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}

/// The question naming a running push and a terminal running a program, with what quitting
/// does to each.
#[test]
fn quit_confirm_terminal_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "quit-confirm-terminal");
    st.job = Some(RunningJob {
        label: "Push".into(),
        workdir: st.tabs[0].path.clone(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    });
    let tab = crate::state::menu_bar::tab_name(&st, 0).to_owned();
    let busy = BusyTerminal { id: 1, tab, terminal: "pwsh \u{b7} quit-confirm-terminal".into(), program: Some("cargo".into()) };
    st.modal = quit_confirm(&st, &[busy]);
    let mut h = harness(st, true);
    h.run_ok();
    h.check_snapshot("quit_confirm_terminal");
    drop(h);
    remove_fixture(&dir);
}
