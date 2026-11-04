//! What update and the shell hand the views each frame (`AppState::cues`): the clock, the
//! window, the hover highlight, the files a change step lands on or reveals, and other state
//! that only update (or the shell) changes. Views read it and never write it; what they may
//! write is in `ViewState`.

use crate::state::window_chrome::{Chrome, WindowInfo};
use crate::state::window_native::Backdrop;
use git2::Oid;
use glowfly_core::repo::DiffTarget;
use std::collections::HashSet;
use std::time::Instant;

/// The update-owned cues the views draw from (`AppState::cues`); views only read them.
pub struct ViewCues {
    /// Put this hunk at the top of the next freshly opened diff (its header row, or its change
    /// block in the whole-file view); set when the view mode changes.
    pub diff_open_hunk: Option<usize>,
    /// A file opened by next/previous change, still loading, and the change to land on. Set
    /// and cleared only by `update`.
    pub diff_landing: Option<(DiffTarget, crate::state::diff_nav::Landing)>,
    /// The file a next/previous change step landed on and the `DiffSlot::shown_gen` it landed at:
    /// the file list it came from scrolls its row into view once. Set and cleared only by
    /// `update`.
    pub file_reveal: Option<(DiffTarget, u64)>,
    /// A file opened (not reloaded) whose diff hasn't arrived yet. Set by `repo::open_diff`;
    /// cleared when a result of the current generation arrives, the diff closes or the tab is
    /// parked. Changed only by `update`.
    pub diff_opening: Option<DiffTarget>,
    /// The open file being staged or unstaged as a whole (alone or with the other selected
    /// files), and the file to open when its operation reports success (`update/wip_list.rs`).
    /// Changed only by `update`.
    pub stage_move_on: Option<crate::state::wip_lists::MoveOn>,
    /// The files a Shift move selected in the Unstaged or Staged Files list. Changed only by
    /// `update`.
    pub wip_span: Option<crate::state::wip_lists::WipSpan>,
    /// The file each of those lists (Unstaged, then Staged) last opened, where going to the
    /// list from the other one lands. Changed only by `update`.
    pub wip_place: [Option<String>; 2],
    /// A file opened from a pull request's code comment thread, still loading, and the line
    /// (side, number) to land on. Set and cleared only by `update`.
    pub diff_open_line: Option<(DiffTarget, glowfly_core::diff_rows::LineSide, u32)>,
    /// The change row next/previous change last moved to. Steps continue from it while it is
    /// on screen, because a change near the end can't scroll to the top.
    pub diff_cursor: Option<usize>,
    /// Branch tip under the pointer (sidebar or BRANCH/TAG column), for row highlighting. The
    /// views record the next one in `ViewState::hover_tip_next`; the shell moves it here.
    pub hover_tip: Option<Oid>,
    /// The hovered ref's tip and the commits on its branch, for row highlighting. Set by
    /// `update` (`GraphMsg::HoverRef`); the graph only reads it.
    pub highlight: Option<(Oid, std::sync::Arc<HashSet<Oid>>)>,
    /// The walks behind `highlight`, kept for the snapshot (`ref_rows::RefRows`); reset when a
    /// new snapshot or tab arrives.
    pub ref_rows: crate::state::ref_rows::RefRows,
    /// The window had the keyboard focus last frame (for the WindowFocused transition).
    pub was_focused: bool,
    /// The window has the keyboard focus this frame. The shell sets it before handling results;
    /// update reads it to decide desktop notifications.
    pub focused: bool,
    /// Wall-clock time (Unix seconds) for this frame, set by the shell; fixed in tests.
    pub now: i64,
    /// The shell's time stamp for this frame (ui/app.rs); toasts fade and expire by it. None in
    /// snapshot tests: toasts stay at full opacity.
    pub frame_at: Option<Instant>,
    /// UTC offset (seconds) used to display times; the local zone in the app, fixed in tests.
    pub utc_offset: i32,
    /// The slots that pass for the trunk tone in the active palette (a bitmask), as the shell
    /// reported them (`GraphMsg::TrunkNear`); `update` folds them into each repository's colouring.
    pub trunk_near: u16,
    /// The recent frames' times, set by the shell after each frame, for the frame-time overlay.
    pub frame_times: crate::state::frame_times::FrameSummary,
    /// Fetched avatars waiting for the shell to turn them into textures.
    pub avatars_loaded: Vec<(String, Option<glowfly_core::avatars::Image>)>,
    /// Lines selected in the open diff for line-level staging (changed only by `update`).
    pub diff_lines: crate::state::lines::LineSelection,
    /// The zoom toast's id, so the next zoom step updates it (set only by update).
    pub zoom_toast: Option<u64>,
    /// The tool view in which the collapsed commit panel was expanded; set and cleared only by
    /// update (`state::layout::drop_stale_expansion`), so it isn't parked with a tab.
    pub details_expanded_for: Option<crate::state::layout::ToolView>,
    /// Who draws the window frame: set once by the real app (`Chrome::chosen`); tests keep
    /// `System` unless they pick another.
    pub chrome: Chrome,
    /// Preferences > UI offers the Windows system title bar setting: set once by the real app on
    /// Windows (`Chrome::offers_system_title_bar`); tests keep it off unless they turn it on.
    pub system_title_bar_offered: bool,
    /// Whether the window is maximized or fullscreen, read by the shell each frame.
    pub window: WindowInfo,
    /// Quit went ahead (update/app.rs): the close request that follows isn't asked about again.
    pub quitting: bool,
    /// What the open quit question named, so it closes the window once all of that has ended
    /// (update/app.rs `settle`); `None` while it isn't open.
    pub quit_ask: Option<crate::state::quit_confirm::QuitAsk>,
    /// Quit anyway asked the running job to stop (a fetch): the window closes once it has
    /// ended, its leftover lock files removed.
    pub quit_after_stop: bool,
    /// What the title bar, toolbar and sidebar are painted on: set by the shell each frame;
    /// views read it through `ui::backdrop::fill`.
    pub backdrop: Backdrop,
    /// The WIP panel's steps opened or folded by hand while an operation is in progress (set
    /// only by update, `LayoutMsg::ToggleOpStep`).
    pub op_steps: crate::state::op_steps::StepFolds,
    /// The system's accessibility settings as last read (set only by update,
    /// `AppMsg::SystemA11y`).
    pub system_a11y: crate::state::system_a11y::SystemA11y,
}

impl Default for ViewCues {
    fn default() -> Self {
        Self {
            diff_open_hunk: None,
            diff_landing: None,
            file_reveal: None,
            diff_opening: None,
            stage_move_on: None,
            wip_span: None,
            wip_place: [None, None],
            diff_open_line: None,
            diff_cursor: None,
            hover_tip: None,
            highlight: None,
            ref_rows: Default::default(),
            was_focused: true,
            focused: true,
            now: 0,
            frame_at: None,
            utc_offset: 0,
            trunk_near: 0,
            frame_times: Default::default(),
            avatars_loaded: Vec::new(),
            diff_lines: crate::state::lines::LineSelection::default(),
            zoom_toast: None,
            details_expanded_for: None,
            chrome: Chrome::System,
            system_title_bar_offered: false,
            window: WindowInfo::default(),
            quitting: false,
            quit_ask: None,
            quit_after_stop: false,
            backdrop: Backdrop::Solid,
            op_steps: Default::default(),
            system_a11y: Default::default(),
        }
    }
}

impl crate::state::AppState {
    /// Drops what is selected in the diff: its lines and its text.
    pub fn clear_diff_selection(&mut self) {
        self.cues.diff_lines.clear();
        self.view.diff_text = None;
    }
}
