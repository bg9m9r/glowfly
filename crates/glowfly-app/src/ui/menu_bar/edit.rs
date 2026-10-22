//! Edit: Undo and Redo, finding commits, and copying the selected commit's SHA or message or the
//! checked-out branch's name.

use super::items::{clicked, hint, item};
use crate::keymap::BindingId as I;
use crate::msg::{JobMsg, Msg, SearchMsg};
use crate::state::menu_bar::{ShaCopy, copy_branch, copy_message, copy_sha};
use crate::state::{AppState, Selection};
use crate::ui::menus::copy_shas;
use eframe::egui;

pub(super) fn items(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.set_min_width(250.0);
    let idle = st.job.is_none();
    let (label, why) = st.undo.undo_entry(t, idle);
    item(ui, &st.keymap, label, Some(I::Undo), why.as_deref(), Msg::Job(JobMsg::Undo), msgs);
    let (label, why) = st.undo.redo_entry(t, idle);
    item(ui, &st.keymap, label, Some(I::Redo), why.as_deref(), Msg::Job(JobMsg::Redo), msgs);
    ui.separator();
    let no_repo = st.snap.is_none().then_some(t.tr("menu.common.no_repo"));
    item(ui, &st.keymap, t.tr("menu.edit.find"), Some(I::Search), no_repo, Msg::Search(SearchMsg::Open), msgs);
    // What the key does: show the bar and flip the filter.
    if clicked(ui, t.tr("menu.edit.filter"), hint(&st.keymap, Some(I::SearchFilter)), no_repo) {
        st.view.show_search = true;
        msgs.push(Msg::Search(SearchMsg::ToggleFilter));
        ui.close_menu();
    }
    ui.separator();
    copy_items(ui, st, msgs);
}

fn copy_items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let keys = &st.keymap;
    let t = st.settings.lang();
    match copy_sha(st) {
        Ok(copy) => {
            let label = copy.label(t);
            let msg = match (copy, &st.selection) {
                (ShaCopy::One(sha), _) => Msg::Copy(sha),
                // Joined as the graph's commit menu joins them.
                (ShaCopy::Many(_), Selection::Commits(m)) => copy_shas(m),
                (ShaCopy::Many(_), _) => Msg::Copy(String::new()),
            };
            item(ui, keys, label, None, None, msg, msgs);
        }
        Err(why) => item(ui, keys, t.tr("menu.edit.copy_sha"), None, Some(why), Msg::Copy(String::new()), msgs),
    }
    let (text, why) = split(copy_message(st));
    item(ui, keys, t.tr("menu.edit.copy_message"), None, why, Msg::Copy(text), msgs);
    let (text, why) = split(copy_branch(st));
    item(ui, keys, t.tr("menu.edit.copy_branch"), None, why, Msg::Copy(text), msgs);
}

/// The text to copy (empty when disabled) and the reason it is disabled.
fn split(r: Result<String, &'static str>) -> (String, Option<&'static str>) {
    match r {
        Ok(text) => (text, None),
        Err(why) => (String::new(), Some(why)),
    }
}
