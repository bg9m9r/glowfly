//! Transient view state: scroll targets, hover highlights, focus requests and which
//! left-panel sections are expanded.

use super::{diff_nav, lines};
use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::DiffTarget;
use trellis_core::settings::ResizableColumn;

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
    pub gitflow: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self {
            local: true,
            remote: true,
            prs: true,
            issues: true,
            tags: false,
            stashes: true,
            submodules: true,
            worktrees: true,
            gitflow: true,
        }
    }
}

/// A graph header divider being dragged, and the preview width the graph lays out with until
/// the release stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnDrag {
    pub col: ResizableColumn,
    /// Pointer x where the drag was pressed.
    pub start_x: f32,
    /// The column's stored width when the drag started.
    pub start_width: f32,
    /// How far the column may grow before the message column gets too narrow.
    pub room: f32,
    pub width: f32,
}

/// Purely visual state that views may change directly.
pub struct ViewState {
    pub show_log: bool,
    pub show_search: bool,
    /// The search bar's query text (parsed by update on `SearchMsg::QueryChanged`).
    pub search: String,
    /// The author picker's filter field.
    pub author_filter: String,
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
    /// A file opened from a pull request's code comment thread, still loading, and the line
    /// (side, number) to land on. Set and cleared only by `update`.
    pub diff_open_line: Option<(DiffTarget, trellis_core::diff_rows::LineSide, u32)>,
    /// Scroll the pull request's comment pane to this thread on the next frame (set by
    /// `update`, taken by the view).
    pub pr_thread_scroll: Option<String>,
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
    /// Branch tips drawn this frame (LOCAL and REMOTE rows, graph ref labels), filled by the
    /// views and taken by the shell.
    pub tips_seen: Vec<Oid>,
    /// The hovered ref's tip and the commits on its branch, for row highlighting. Set by
    /// `update` (`GraphMsg::HoverRef`); the graph only reads it.
    pub highlight: Option<(Oid, HashSet<Oid>)>,
    /// The commits reachable from the default branch, walked on the first hover after a load
    /// (`ref_rows::default_ancestry`); cleared when a new snapshot or tab arrives.
    pub base_set: Option<HashSet<Oid>>,
    /// The window had the keyboard focus last frame (for the WindowFocused transition).
    pub was_focused: bool,
    /// The window has the keyboard focus this frame. The shell sets it before handling results;
    /// update reads it to decide desktop notifications.
    pub focused: bool,
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
    /// The header divider being dragged (set and cleared by the graph view).
    pub column_drag: Option<ColumnDrag>,
    /// The zoom toast's id, so the next zoom step updates it (set only by update).
    pub zoom_toast: Option<u64>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            show_log: false,
            show_search: false,
            search: String::new(),
            author_filter: String::new(),
            ref_filter: String::new(),
            sections: Sections::default(),
            scroll_to_row: None,
            scroll_offset: None,
            graph_scroll: (0.0, 0.0),
            diff_top_row: 0,
            diff_scroll_to_row: None,
            diff_open_hunk: None,
            diff_landing: None,
            diff_open_line: None,
            pr_thread_scroll: None,
            diff_cursor: None,
            diff_expanded: Default::default(),
            diff_visible_rows: 0,
            hover_tip: None,
            hover_tip_next: None,
            tips_seen: Vec::new(),
            highlight: None,
            base_set: None,
            was_focused: true,
            focused: true,
            was_typing: false,
            now: 0,
            utc_offset: 0,
            avatars_loaded: Vec::new(),
            diff_lines: lines::LineSelection::default(),
            conflict_scroll_to: None,
            history_scroll_to: None,
            focus_search: false,
            focus_message: false,
            column_drag: None,
            zoom_toast: None,
        }
    }
}
