//! A reload's scroll anchor (`state/graph_anchor.rs`): taken from the rows on screen before the
//! new snapshot replaces them, and applied once it is shown, unless a scroll was asked for
//! meanwhile.

use crate::effect::LoadKind;
use crate::state::AppState;
use crate::state::graph_anchor::{GraphAnchor, Pending};
use crate::state::graph_rows::GraphRows;

/// The graph's scroll requests not yet drawn.
fn pending(st: &AppState) -> Pending {
    (st.view.scroll_to_row, st.view.scroll_offset)
}

/// The anchor for a reload of kind `kind`: None for an opening, a freed history's stub, or a
/// graph never drawn.
pub(super) fn take(st: &AppState, kind: LoadKind) -> Option<GraphAnchor> {
    if kind == LoadKind::Open || st.session.tab_view.history_freed.is_some() {
        return None;
    }
    let snap = st.snap.as_ref()?;
    let row_h = st.view.tab.graph_row_h;
    // A scroll asked for but not yet drawn is where the graph is about to be.
    let offset = match pending(st) {
        (Some(row), _) => row as f32 * row_h,
        (None, Some(off)) => off,
        (None, None) => st.view.tab.graph_scroll.0,
    };
    let rows = GraphRows::new(snap, &st.session.search, st.view.show_search);
    GraphAnchor::take(&rows, snap, offset, row_h, pending(st))
}

/// Puts the anchor's commit back where it was on screen, unless something asked to scroll since
/// or it didn't move.
pub(super) fn restore(st: &mut AppState, anchor: Option<GraphAnchor>) {
    let Some(a) = anchor else { return };
    if !a.still_wanted(pending(st)) {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let rows = GraphRows::new(snap, &st.session.search, st.view.show_search);
    let Some(off) = a.offset_in(&rows).filter(|off| (off - a.offset()).abs() >= 0.5) else { return };
    st.view.scroll_to_row = None;
    st.view.scroll_offset = Some(off);
    // A second reload or a park before the next frame starts from here.
    st.view.tab.graph_scroll.0 = off;
}
