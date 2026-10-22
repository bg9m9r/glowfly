//! Transient view state: scroll targets, hover highlights, focus requests and which
//! left-panel sections are expanded.

use super::window_chrome::{AltTap, Chrome, WindowInfo};
use super::window_native::Backdrop;
use super::{diff_nav, lines};
use git2::Oid;
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::{ResizableColumn, SizedPanel};
use std::collections::HashSet;
use std::time::Instant;

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
    /// An activity row to scroll into view once (set by update for a toast's Details; the
    /// view clears it).
    pub activity_reveal: Option<crate::state::logs::EntryId>,
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
    /// Graph rows that fit the viewport in the last frame, recorded by the view.
    pub graph_fit_rows: usize,
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
    /// The file a next/previous change step landed on and the `diff_shown_gen` it landed at:
    /// the file list it came from scrolls its row into view once. Set and cleared only by
    /// `update`.
    pub file_reveal: Option<(DiffTarget, u64)>,
    /// A file opened (not reloaded) whose diff hasn't arrived yet. Set by `repo::open_diff`;
    /// cleared when a result of the current generation arrives, the diff closes or the tab is
    /// parked. Changed only by `update`.
    pub diff_opening: Option<DiffTarget>,
    /// The file S or U is staging or unstaging as a whole, so that when its operation reports
    /// success the view moves on to its neighbour (`update/wip_list.rs`). Changed only by
    /// `update`.
    pub stage_move_on: Option<DiffTarget>,
    /// A file opened from a pull request's code comment thread, still loading, and the line
    /// (side, number) to land on. Set and cleared only by `update`.
    pub diff_open_line: Option<(DiffTarget, glowfly_core::diff_rows::LineSide, u32)>,
    /// Scroll the pull request's comment pane to this thread on the next frame (set by
    /// `update`, taken by the view).
    pub pr_thread_scroll: Option<String>,
    /// The change row next/previous change last moved to. Steps continue from it while it is
    /// on screen, because a change near the end can't scroll to the top.
    pub diff_cursor: Option<usize>,
    /// The unchanged runs opened in the open diff (whole-file folds and the hunk view's gaps), by
    /// key, with the gaps' text. Changed only by `update`; cleared when a different target's diff
    /// arrives or the diff closes; parked with its tab.
    pub diff_expanded: glowfly_core::whole_file::Expanded,
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
    /// The shell's time stamp for this frame (ui/app.rs); toasts fade and expire by it. None in
    /// snapshot tests: toasts stay at full opacity.
    pub frame_at: Option<Instant>,
    /// UTC offset (seconds) used to display times; the local zone in the app, fixed in tests.
    pub utc_offset: i32,
    /// Fetched avatars waiting for the shell to turn them into textures.
    pub avatars_loaded: Vec<(String, Option<glowfly_core::avatars::Image>)>,
    /// Lines selected in the open diff for line-level staging (changed only by `update`).
    pub diff_lines: lines::LineSelection,
    /// Scroll the conflict panes and output to this chunk on the next frame (set by `update`).
    pub conflict_scroll_to: Option<usize>,
    /// Scroll the file history list to this entry on the next frame (set by `update`).
    pub history_scroll_to: Option<usize>,
    /// Scroll the folder entry's file list to this file on the next frame (set by update after
    /// a step).
    pub history_file_scroll: Option<usize>,
    /// Focus the graph search field on the next frame (set by update).
    pub focus_search: bool,
    /// Focus the commit summary field on the next frame (set by update, taken by the commit box).
    pub focus_message: bool,
    /// The header divider being dragged (set and cleared by the graph view).
    pub column_drag: Option<ColumnDrag>,
    /// The drawer whose edge is being dragged (set and cleared by `ui::panel_size::track`).
    pub panel_drag: Option<SizedPanel>,
    /// The zoom toast's id, so the next zoom step updates it (set only by update).
    pub zoom_toast: Option<u64>,
    /// The tool view in which the collapsed commit panel was expanded; set and cleared only by
    /// update (`state::layout::drop_stale_expansion`), so it isn't parked with a tab.
    pub details_expanded_for: Option<super::layout::ToolView>,
    /// Who draws the window frame: set once by the real app (`Chrome::current`); tests keep
    /// `System` unless they pick another.
    pub chrome: Chrome,
    /// Whether the window is maximized or fullscreen, read by the shell each frame.
    pub window: WindowInfo,
    /// The Alt tap that opens a drawn title bar's menu (`ui/title_bar.rs`).
    pub alt_tap: AltTap,
    /// What the title bar, toolbar and sidebar are painted on: set by the shell each frame;
    /// views read it through `ui::backdrop::fill`.
    pub backdrop: Backdrop,
    /// The WIP panel's steps opened or folded by hand while an operation is in progress (set
    /// only by update, `LayoutMsg::ToggleOpStep`).
    pub op_steps: super::op_steps::StepFolds,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            show_log: false,
            activity_reveal: None,
            show_search: false,
            search: String::new(),
            author_filter: String::new(),
            ref_filter: String::new(),
            sections: Sections::default(),
            scroll_to_row: None,
            scroll_offset: None,
            graph_scroll: (0.0, 0.0),
            graph_fit_rows: 0,
            diff_top_row: 0,
            diff_scroll_to_row: None,
            diff_open_hunk: None,
            diff_landing: None,
            file_reveal: None,
            diff_opening: None,
            stage_move_on: None,
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
            frame_at: None,
            utc_offset: 0,
            avatars_loaded: Vec::new(),
            diff_lines: lines::LineSelection::default(),
            conflict_scroll_to: None,
            history_scroll_to: None,
            history_file_scroll: None,
            focus_search: false,
            focus_message: false,
            column_drag: None,
            panel_drag: None,
            zoom_toast: None,
            details_expanded_for: None,
            chrome: Chrome::System,
            window: WindowInfo::default(),
            alt_tap: AltTap::default(),
            backdrop: Backdrop::Solid,
            op_steps: Default::default(),
        }
    }
}
