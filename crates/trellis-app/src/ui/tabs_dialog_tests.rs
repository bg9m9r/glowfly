//! The tab dialogs: Rename tab, and the confirmation before closing tabs that hold unsaved work.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{CloseRequest, Msg, TabsMsg};
use crate::state::conflict::ConflictView;
use crate::state::tab_losses::close_confirm;
use crate::state::tabs::{Parked, RenameTabForm};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::conflict_pick::ChunkPick;
use trellis_core::repo::Choice;
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn label_set(h: &H) -> Option<Option<String>> {
    h.state().1.iter().find_map(|m| if let Msg::Tabs(TabsMsg::SetTabLabel { label, .. }) = m { Some(label.clone()) } else { None })
}

#[test]
fn tabs_rename_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-rename");
    let mut form = RenameTabForm::new(&st.tabs[1]);
    form.text = "Frontend".into();
    st.modal = Some(Modal::RenameTab(form));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_rename_dialog");
    assert!(h.query_by_label("Use folder name").is_some());
    h.get_by_label("Rename").click();
    h.run();
    assert_eq!(label_set(&h), Some(Some("Frontend".to_owned())));
    drop(h);

    // With a label set, Use folder name clears it.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "tabs-rename");
    st.tabs[1].label = Some("API".into());
    st.modal = Some(Modal::RenameTab(RenameTabForm::new(&st.tabs[1])));
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Use folder name").click();
    h.run();
    assert_eq!(label_set(&h), Some(None));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tabs_close_confirm() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-close-confirm");
    st.draft.summary = "Fix the login form".into();
    let mut conflict = ConflictView::loading("src/lib.rs".into());
    conflict.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs), ChunkPick::default()];
    conflict.auto = vec![None, None, None];
    st.tabs[1].parked = Some(Box::new(Parked { conflict: Some(conflict), ..Default::default() }));
    let then = Msg::Tabs(TabsMsg::CloseConfirmed(CloseRequest::Others(st.tabs[0].id)));
    st.modal = close_confirm(&st, &[0, 1], then);
    assert!(st.modal.is_some());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_close_confirm");
    h.get_by_label("Close 2 tabs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}
