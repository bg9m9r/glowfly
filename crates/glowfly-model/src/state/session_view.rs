//! The per-tab view state that parks with its tab inside `RepoSession::tab_view`: the search
//! bar's text, the sidebar's ref filter, the graph's scroll, the folders collapsed in the
//! changed-file lists, and the open diff's opened folds and top row with the mode and layout that
//! row was counted in. Views edit the text fields in place and the graph view records its scroll;
//! `update/tabs.rs` restores the scroll and recounts the top row on return.

use super::file_folds::FileFolds;
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::Expanded;

/// A tab's view state (`RepoSession::tab_view`).
#[derive(Default)]
pub struct TabView {
    /// The search bar's query text (parsed by update on `SearchMsg::QueryChanged`).
    pub search: String,
    /// The sidebar's ref filter field.
    pub ref_filter: String,
    /// The graph's scroll offset and viewport height from the last frame, recorded by the view;
    /// the graph scrolls back to the offset on return.
    pub graph_scroll: (f32, f32),
    /// Folders collapsed in the changed-file lists' tree, per list (changed only by update,
    /// `LayoutMsg::ToggleFolder`); never saved.
    pub file_folds: FileFolds,
    /// The unchanged runs opened in the open diff (whole-file folds and the hunk view's gaps), by
    /// key, with the gaps' text. Changed only by `update`; cleared when a different target's diff
    /// arrives or the diff closes.
    pub diff_expanded: Expanded,
    /// First diff row visible in the last frame, recorded by the view (with `diff_expanded`'s
    /// folds open).
    pub diff_top_row: usize,
    /// The diff view mode and layout when the tab was parked: what `diff_top_row` was counted in
    /// (both settings are shared by all tabs). Set by `update/tabs.rs::park`, taken on return;
    /// None while active.
    pub parked_in: Option<(DiffMode, DiffLayout)>,
}
