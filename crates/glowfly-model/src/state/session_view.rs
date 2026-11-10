//! The per-tab view state that parks with its tab inside `RepoSession::tab_view`: the folders
//! collapsed in the changed-file lists and the open diff's opened folds with the mode and layout
//! its top row was counted in, which only update changes. What the views write for the active
//! tab (the graph's scroll and row height and the diff's top row, the search bar's text, the sidebar's ref
//! filter and the empty sections expanded) is `TabViewState` (`ViewState::tab`), parked beside
//! the session (`Parked::view`); `update/tabs.rs` restores the scroll and recounts the top row on
//! return. Also whether the tab's history was freed while parked.

use super::file_folds::FileFolds;
use empty_sections::EmptySections;
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::Expanded;

#[path = "empty_sections.rs"]
pub mod empty_sections;

/// What the views write for the active tab (`ViewState::tab`); `update/tabs.rs` parks it with
/// the tab (`Parked::view`).
#[derive(Default)]
pub struct TabViewState {
    /// The graph's scroll offset and viewport height from the last frame, recorded by the view;
    /// the graph scrolls back to the offset on return.
    pub graph_scroll: (f32, f32),
    /// The graph's row height in the last frame, recorded by the view with `graph_scroll`; 0
    /// until the graph is drawn. Reloads anchor the scroll with it (`state/graph_anchor.rs`).
    pub graph_row_h: f32,
    /// First diff row visible in the last frame, recorded by the view (with `TabView::diff_expanded`'s
    /// folds open).
    pub diff_top_row: usize,
    /// The search bar's query text (parsed by update on `SearchMsg::QueryChanged`).
    pub search: String,
    /// The sidebar's ref filter field.
    pub ref_filter: String,
    /// The left-panel sections expanded while empty (`empty_sections.rs`).
    pub empty_sections: EmptySections,
}

/// A tab's view state (`RepoSession::tab_view`).
#[derive(Default)]
pub struct TabView {
    /// Folders collapsed in the changed-file lists' tree, per list (changed only by update,
    /// `LayoutMsg::ToggleFolder`); never saved.
    pub file_folds: FileFolds,
    /// The unchanged runs opened in the open diff (whole-file folds and the hunk view's gaps), by
    /// key, with the gaps' text. Changed only by `update`; cleared when a different target's diff
    /// arrives or the diff closes.
    pub diff_expanded: Expanded,
    /// The diff view mode and layout when the tab was parked: what `TabViewState::diff_top_row` was counted in
    /// (both settings are shared by all tabs). Set by `update/tabs.rs::park`, taken on return;
    /// None while active.
    pub parked_in: Option<(DiffMode, DiffLayout)>,
    /// Set while the snapshot is a stub whose commits and graph were freed after a long park
    /// (`update/parked_free.rs`): how many commits it had been loaded with, which the reload on
    /// return asks for again. Cleared when a load's snapshot is shown.
    pub history_freed: Option<usize>,
    /// Why the last reload of a freed history failed, for the graph's pane (with Try again);
    /// cleared with `history_freed`.
    pub history_reload_error: Option<String>,
}
