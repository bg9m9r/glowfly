//! The moves of a focused list that only opens files (`ui/file_list_keys.rs` reads them): Up/Down,
//! Page Up/Page Down and Home/End open the file they land on in the list's on-screen order
//! (`state/file_lists.rs`), as a click on its row would, and have the list scroll it into view.
//! From no current file every move but End starts at the first file. A key never leaves a
//! conflict view holding picks that aren't saved (the log says why); a click still does.

use crate::effect::Effect;
use crate::msg::{ConflictMsg, HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::file_lists::{Opens, listing};
use crate::state::list_keys::{FileList, ListMove, place_in};
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, list: FileList, mv: ListMove, now: Instant, fx: &mut Vec<Effect>) {
    let Some(l) = listing(st, list) else { return };
    let visible: Vec<usize> = l.order.iter().filter(|(_, shown)| *shown).map(|(i, _)| *i).collect();
    let Some(to) = mv.target(visible.len(), place_in(&l.order, l.current)) else { return };
    if let Some(c) = st.session.conflict.view.as_ref().filter(|c| c.user_choices() > 0) {
        let text = st.settings.lang().trf("conflict.files.keys_kept", &[("path", &c.path)]);
        st.log(false, text);
        return;
    }
    match l.opens[visible[to]].clone() {
        Opens::Diff(t) => super::wip_list::open(st, t, fx),
        Opens::Conflict(path) => {
            fx.extend(super::update_sealed(st, Msg::Conflict(ConflictMsg::OpenConflict(path.clone())), now));
            // Conflicted Files finds its row by the file's working-tree target.
            st.view.file_reveal = Some((DiffTarget::Unstaged(path), st.session.conflict.generation));
        }
        Opens::FolderFile(i) => {
            fx.extend(super::update_sealed(st, Msg::History(HistoryMsg::SelectHistoryFile(i)), now));
            st.view.history_file_scroll = Some(i);
        }
    }
}
