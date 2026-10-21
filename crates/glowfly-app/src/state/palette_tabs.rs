//! The command palette's tab commands: new, reopen (the newest closed tab, or any one on the
//! recently closed list), close, rename and move the active tab.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};

/// The tab commands available now.
pub(super) fn tab_entries(st: &AppState, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    out.push(entry(C::NewTab, "New tab", "add open tab"));
    if let Some(last) = st.closed_tabs.last() {
        let e = entry(C::ReopenClosedTab, "Reopen closed tab", "restore undo closed recent tab");
        out.push(with_detail(e, Some(last.name())));
    }
    if st.snap.is_some() || st.tabs.len() > 1 {
        out.push(entry(C::CloseTab, "Close tab", "close repository"));
    }
    let n = st.tabs.len();
    if st.tabs.get(st.active_tab).is_some_and(|t| t.path.is_some()) {
        out.push(entry(C::RenameTab, "Rename tab…", "label alias name tab title"));
    }
    if st.active_tab > 0 && st.active_tab < n {
        out.push(entry(C::MoveTab(-1), "Move tab left", "reorder order tab"));
    }
    if st.active_tab + 1 < n {
        out.push(entry(C::MoveTab(1), "Move tab right", "reorder order tab"));
    }
    for closed in st.closed_tabs.iter().rev() {
        let e = entry(C::ReopenClosed(closed.path.clone()), format!("Reopen closed tab: {}", closed.name()), "restore closed recent tab");
        out.push(with_detail(e, Some(glowfly_core::native_path::shown(&closed.path))));
    }
}
