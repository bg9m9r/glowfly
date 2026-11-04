//! Transient view state the views write: scroll targets and positions, focus requests, hover,
//! drags, which left-panel sections are expanded and the text fields moved out of the model. What
//! only update or the shell sets for the views is in `view_cues.rs`. `AppState::with_view` lends
//! it to the views apart from the rest of `AppState`, so they read the model and write only this.

use super::window_chrome::AltTap;
use git2::Oid;
use glowfly_core::settings::{ResizableColumn, SizedPanel};

#[path = "diff_text.rs"]
pub mod diff_text;
#[path = "file_folds.rs"]
pub mod file_folds;
#[path = "file_lists.rs"]
pub mod file_lists;
#[path = "file_tree.rs"]
pub mod file_tree;
/// The shell's frame timing, which fills `ViewCues::frame_times`.
#[path = "frame_times.rs"]
pub mod frame_times;
/// `Lent`, which holds `AppState::view` while `with_view` lends it out.
#[path = "lent.rs"]
pub mod lent;
#[path = "list_keys.rs"]
pub mod list_keys;
#[path = "system_a11y.rs"]
pub mod system_a11y;
#[path = "view_cues.rs"]
mod view_cues;
#[path = "wip_lists.rs"]
pub mod wip_lists;

pub use view_cues::ViewCues;

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

/// Purely visual state that views may change directly: scroll requests and positions, focus
/// requests, hover, drags and expanded sections. What only update or the shell sets is in
/// `ViewCues` (`AppState::cues`).
pub struct ViewState {
    pub show_log: bool,
    /// An activity row to scroll into view once (set by update for a toast's Details; the
    /// view clears it).
    pub activity_reveal: Option<crate::state::logs::EntryId>,
    pub show_search: bool,
    /// The author picker's filter field.
    pub author_filter: String,
    /// The dashboard's filter field (cleared by `DashboardMsg::ClearFilter`).
    pub dashboard_filter: String,
    pub sections: Sections,
    /// Scroll the graph so this row is near the top on the next frame (set by `update`).
    pub scroll_to_row: Option<usize>,
    /// Scroll the graph to this pixel offset on the next frame (keyboard navigation).
    pub scroll_offset: Option<f32>,
    /// Graph rows that fit the viewport in the last frame, recorded by the view.
    pub graph_fit_rows: usize,
    /// Scroll the diff so this row is at the top on the next frame (set by `update`).
    pub diff_scroll_to_row: Option<usize>,
    /// Give that list (`Some(staged)`) the keyboard focus on the next frame (set by `update`,
    /// taken by the view).
    pub wip_focus: Option<bool>,
    /// Give that pane the keyboard focus (set by `update` for the pane keys, Enter in the graph
    /// and F6; taken by the view that draws the pane).
    pub pane_focus: Option<super::layout::Pane>,
    /// A graph key for the graph view to apply with its rows at hand (set by the shortcuts).
    pub graph_key: Option<super::graph_rows::GraphKey>,
    /// Scroll the pull request's comment pane to this thread on the next frame (set by
    /// `update`, taken by the view).
    pub pr_thread_scroll: Option<String>,
    /// Focus the issue panel's comment field, with the cursor at the end, on the next frame
    /// (set by `update` after a Reply quoted a comment, taken by the view).
    pub issue_comment_focus: bool,
    /// Focus the pull request's new code comment box and scroll its pane to it on the next
    /// frame (set by `update`, taken by the view).
    pub pr_line_comment_focus: bool,
    /// How many diff rows fit in the viewport, recorded by the view.
    pub diff_visible_rows: usize,
    pub hover_tip_next: Option<Oid>,
    /// Branch tips drawn this frame (LOCAL and REMOTE rows, graph ref labels), filled by the
    /// views and taken by the shell.
    pub tips_seen: Vec<Oid>,
    /// The LOCAL section's rows, kept between frames.
    pub local_rows: crate::state::stack_rows::LocalRowsCache,
    /// A text field had the keyboard when the last frame was drawn. egui takes the focus
    /// away when Esc is pressed before the shell sees the key, so Esc checks this instead.
    pub was_typing: bool,
    /// The keyboard was on the Preferences Save / Revert bar (its Save or Revert button) when
    /// the last frame was drawn: Esc there reverts the page.
    pub prefs_bar_focused: bool,
    /// Text selected in the open diff's code: written by the view as the pointer drags,
    /// cleared by update when another diff (or load of it) shows or the layout switches.
    pub diff_text: Option<diff_text::TextSel>,
    /// Rows and columns short of a whole one that dragging a text selection past the diff's
    /// edge has scrolled, carried to the next frame; view state.
    pub diff_text_edge: [f32; 2],
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
    /// The Alt tap that opens a drawn title bar's menu (`ui/title_bar.rs`).
    pub alt_tap: AltTap,
    /// The menus' keyboard state: a mnemonic pressed, a menu opened from the keyboard
    /// (`ui/menu_bar`).
    pub menu_keys: super::menu_keys::MenuKeys,
    /// The terminal panel's keyboard, wheel, drag and clipboard state between frames.
    pub terminal: crate::state::terminal::TerminalView,
    /// The active tab's graph scroll and diff top row, recorded by the views; parked with the tab
    /// (`Parked::view`).
    pub tab: crate::state::session_view::TabViewState,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            show_log: false,
            activity_reveal: None,
            show_search: false,
            author_filter: String::new(),
            dashboard_filter: String::new(),
            sections: Sections::default(),
            scroll_to_row: None,
            scroll_offset: None,
            graph_fit_rows: 0,
            diff_scroll_to_row: None,
            wip_focus: None,
            pane_focus: None,
            graph_key: None,
            pr_thread_scroll: None,
            issue_comment_focus: false,
            pr_line_comment_focus: false,
            diff_visible_rows: 0,
            hover_tip_next: None,
            tips_seen: Vec::new(),
            local_rows: Default::default(),
            was_typing: false,
            prefs_bar_focused: false,
            diff_text: None,
            diff_text_edge: [0.0; 2],
            conflict_scroll_to: None,
            history_scroll_to: None,
            history_file_scroll: None,
            focus_search: false,
            focus_message: false,
            column_drag: None,
            panel_drag: None,
            alt_tap: AltTap::default(),
            menu_keys: Default::default(),
            terminal: Default::default(),
            tab: Default::default(),
        }
    }
}

impl super::AppState {
    /// Runs `draw` with the rest of the state to read and `ViewState` to write, borrowed apart:
    /// how views are given state, so the compiler keeps them from changing the model. Meanwhile
    /// `self.view` is lent (`lent.rs`): `draw` reads the view state it is given.
    pub fn with_view<R>(&mut self, draw: impl FnOnce(&Self, &mut ViewState) -> R) -> R {
        let mut view = self.view.lend();
        let out = draw(self, &mut view);
        self.view.give_back(view);
        out
    }
}
