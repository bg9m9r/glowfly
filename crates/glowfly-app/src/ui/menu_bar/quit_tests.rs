//! The quit question while a push runs, as the window draws it: the danger confirmation with
//! Cancel focused and Quit anyway on the right, naming the job and its tab (and, in the second,
//! a terminal running a program; in the third, unsaved work too, over the edited form that stays
//! beneath it).

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

/// The question naming unsaved work (a commit message draft, conflict choices and an edited
/// New branch dialog, which stays beneath it), a running push and a busy terminal, in one
/// question over the dimmed form.
#[test]
fn quit_confirm_unsaved_dialog() {
    use crate::state::Modal;
    use crate::state::conflict::ConflictView;
    use crate::state::dialog_guard::open_over;
    use glowfly_core::conflict_pick::ChunkPick;
    use glowfly_core::repo::Choice;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "quit-confirm-unsaved");
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
    st.session.draft.summary = "fix: keep the login".into();
    let mut conflict = ConflictView::loading("src/login.rs".into());
    conflict.picks = vec![ChunkPick::Sides(Choice::Ours)];
    conflict.auto = vec![None];
    st.session.conflict.view = Some(conflict);
    st.modal = Some(Modal::NewBranch { start: None, name: String::new(), checkout: true, touched: Default::default() });
    st.dialog.track(st.modal.as_ref());
    if let Some(Modal::NewBranch { name, .. }) = st.modal.as_mut() {
        *name = "feature/login".into();
    }
    let tab = crate::state::menu_bar::tab_name(&st, 0).to_owned();
    let busy = BusyTerminal { id: 1, tab, terminal: "pwsh \u{b7} quit-confirm-unsaved".into(), program: Some("cargo".into()) };
    let question = quit_confirm(&st, &[busy]).expect("a question");
    open_over(&mut st.modal, &mut st.dialog, question);
    assert!(st.dialog.under().is_some());
    let mut h = harness(st, true);
    h.run_ok();
    h.check_snapshot("quit_confirm_unsaved");
    // The form's Cancel is drawn first, the question's last.
    let cancel = h.get_all_by_label("Cancel").last().expect("the question's Cancel");
    assert!(cancel.is_focused(), "the question has the keyboard, not the form beneath");
    h.get_by_label("Quit anyway").click();
    h.run_ok();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}
