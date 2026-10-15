//! Transient view state: scroll targets, hover highlights, focus requests and which
//! left-panel sections are expanded.

use super::{diff_nav, lines};
use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::DiffTarget;

/// Each left-panel section's open state; views toggle them directly.
pub struct Sections {
    pub local: bool,
    pub remote: bool,
    pub prs: bool,
    pub issues: bool,
    pub tags: bool,
    pub stashes: bool,
    pub submodules: bool,
    pub worktrees: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self { local: true, remote: true, prs: true, issues: true, tags: false, stashes: true, submodules: true, worktrees: true }
    }
}

/// Purely visual state that views may change directly.
pub struct ViewState {
    pub show_log: bool,
    pub show_search: bool,
    pub search: String,
    pub search_cursor: usize,
    pub ref_filter: String,
    pub sections: Sections,
    /// Scroll the graph so this row is near the top on the next frame (set by `update`).
    pub scroll_to_row: Option<usize>,
    /// Scroll the graph to this pixel offset on the next frame (keyboard navigation).
    pub scroll_offset: Option<f32>,
    /// Graph scroll offset and viewport height from the last frame.
    pub graph_scroll: (f32, f32),
    /// First diff row visible in the last frame, recorded by the view.
    pub diff_top_row: usize,
    /// Scroll the diff so this row is at the top on the next frame (set by `update`).
    pub diff_scroll_to_row: Option<usize>,
    /// Put this hunk at the top of the next freshly opened diff (its header row, or its change
    /// block in the whole-file view); set when the view mode changes.
    pub diff_open_hunk: Option<usize>,
    /// A file opened by next/previous change, still loading, and the change to land on. Set
    /// and cleared only by `update`.
    pub diff_landing: Option<(DiffTarget, diff_nav::Landing)>,
    /// The change row next/previous change last moved to. Steps continue from it while it is
    /// on screen, because a change near the end can't scroll to the top.
    pub diff_cursor: Option<usize>,
    /// The folds opened in the whole-file view of the open diff, by key. Changed only by
    /// `update`; cleared when a different target's diff arrives or the diff closes; parked with
    /// its tab.
    pub diff_expanded: trellis_core::whole_file::Expanded,
    /// How many diff rows fit in the viewport, recorded by the view.
    pub diff_visible_rows: usize,
    /// Branch tip under the pointer (sidebar or BRANCH/TAG column), for row highlighting.
    pub hover_tip: Option<Oid>,
    pub hover_tip_next: Option<Oid>,
    /// The hovered ref's tip and the commits on its branch, for row highlighting. Set by
    /// `update` (`GraphMsg::HoverRef`); the graph only reads it.
    pub highlight: Option<(Oid, HashSet<Oid>)>,
    /// The commits reachable from the default branch, walked on the first hover after a load
    /// (`ref_rows::default_ancestry`); cleared when a new snapshot or tab arrives.
    pub base_set: Option<HashSet<Oid>>,
    pub was_focused: bool,
    /// A text field had the keyboard when the last frame was drawn. egui takes the focus
    /// away when Esc is pressed before the shell sees the key, so Esc checks this instead.
    pub was_typing: bool,
    /// Wall-clock time (Unix seconds) for this frame, set by the shell; fixed in tests.
    pub now: i64,
    /// UTC offset (seconds) used to display times; the local zone in the app, fixed in tests.
    pub utc_offset: i32,
    /// Fetched avatars waiting for the shell to turn them into textures.
    pub avatars_loaded: Vec<(String, Option<trellis_core::avatars::Image>)>,
    /// Lines selected in the open diff for line-level staging (changed only by `update`).
    pub diff_lines: lines::LineSelection,
    /// Scroll the conflict panes and output to this chunk on the next frame (set by `update`).
    pub conflict_scroll_to: Option<usize>,
    /// Scroll the file history list to this entry on the next frame (set by `update`).
    pub history_scroll_to: Option<usize>,
    /// Focus the graph search field on the next frame (set by update).
    pub focus_search: bool,
    /// Focus the commit summary field on the next frame (set by update, taken by the commit box).
    pub focus_message: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            show_log: false,
            show_search: false,
            search: String::new(),
            search_cursor: 0,
            ref_filter: String::new(),
            sections: Sections::default(),
            scroll_to_row: None,
            scroll_offset: None,
            graph_scroll: (0.0, 0.0),
            diff_top_row: 0,
            diff_scroll_to_row: None,
            diff_open_hunk: None,
            diff_landing: None,
            diff_cursor: None,
            diff_expanded: Default::default(),
            diff_visible_rows: 0,
            hover_tip: None,
            hover_tip_next: None,
            highlight: None,
            base_set: None,
            was_focused: true,
            was_typing: false,
            now: 0,
            utc_offset: 0,
            avatars_loaded: Vec::new(),
            diff_lines: lines::LineSelection::default(),
            conflict_scroll_to: None,
            history_scroll_to: None,
            focus_search: false,
            focus_message: false,
        }
    }
}
