//! Tab labels: the Rename tab dialog and the name it gives a tab (shown in the tab bar, the
//! Window menu, the tab list and the window title, and saved with the open tabs).

use crate::effect::{Effect, UiEffect};
use crate::state::tabs::{RenameTabForm, TabId, clean_label};
use crate::state::{AppState, Modal};

/// Opens the Rename tab dialog for tab `i`; an empty tab has nothing to rename.
pub(super) fn dialog(st: &mut AppState, i: usize) {
    if let Some(tab) = st.tabs.get(i).filter(|t| t.path.is_some()) {
        st.modal = Some(Modal::RenameTab(RenameTabForm::new(tab)));
    }
}

/// Gives tab `id` the label `label` (cleaned again; `None` shows the folder name) and closes
/// the dialog.
pub(super) fn set(st: &mut AppState, id: TabId, label: Option<String>, fx: &mut Vec<Effect>) {
    if matches!(st.modal, Some(Modal::RenameTab(_))) {
        st.modal = None;
    }
    let Some(i) = st.tabs.iter().position(|t| t.id == id) else { return };
    let tab = &mut st.tabs[i];
    tab.label = label.and_then(|l| clean_label(&l, &tab.name));
    fx.push(super::tabs::save(st));
    if i == st.active_tab {
        fx.push(Effect::Ui(UiEffect::SetTitle(super::tabs::title(st, i))));
    }
}
