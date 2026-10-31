//! The slow parts of the diff header, kept between frames: the change rows behind Previous, Next
//! and the counter, the added and removed line counts, and the file's place in its list. The rows
//! and counts walk every line of the diff and the order clones every path of the list, so they are
//! built again only when the diff (its generation or contents), the view mode, the layout, the
//! open folds or the LFS summary change (`DiffKey`), or the list, the file or the list mode
//! (`file_order::ListKey`); the rest of the header is built each frame from them.

use crate::state::AppState;
use crate::state::diff_header::{self, HeaderModel};
use crate::state::diff_nav::file_order::{self, FileOrder, ListKey};
use crate::state::lfs_notes;
use glowfly_core::repo::FileDiff;
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::{self, Expanded, WholeFile};

/// What the change rows and line counts were built from. The diff is shared (`Arc`), so its hunks
/// and whole file keep their addresses while it is on screen; a reload comes with a new
/// generation.
#[derive(Clone, Copy, PartialEq)]
struct DiffKey {
    generation: (u64, u64),
    hunks: usize,
    whole: Option<usize>,
    mode: DiffMode,
    layout: DiffLayout,
    lfs_summary: bool,
}

/// The header's parts that are built once per change.
#[derive(Default)]
pub struct HeaderCache {
    /// With the open folds they were counted with.
    diff_key: Option<(DiffKey, Expanded)>,
    list_key: Option<ListKey>,
    /// The rows Previous and Next stop at (none while the LFS summary shows).
    pub rows: Vec<usize>,
    /// The diff's '+' and '-' lines.
    pub counts: (usize, usize),
    /// The open file's place in the list it came from.
    pub order: FileOrder,
    /// How many times the rows and counts, and the file order, were built (for tests).
    pub row_builds: usize,
    pub list_builds: usize,
}

impl HeaderCache {
    /// The header of the diff on screen, rebuilding only the parts whose inputs changed.
    pub fn model(&mut self, st: &AppState) -> Option<HeaderModel> {
        self.refresh(st);
        diff_header::build(st, self)
    }

    /// Rebuilds the parts whose inputs changed since the last call; clears them without a diff.
    pub fn refresh(&mut self, st: &AppState) {
        let Some((target, diff)) = st.session.diff.view.as_ref() else {
            *self = HeaderCache { row_builds: self.row_builds, list_builds: self.list_builds, ..Default::default() };
            return;
        };
        let key = diff_key(st, diff);
        let expanded = &st.session.tab_view.diff_expanded;
        if !self.diff_key.as_ref().is_some_and(|(k, ex)| *k == key && ex == expanded) {
            self.rows = crate::state::diff_nav::shown_change_rows(st);
            self.counts = diff_header::line_counts(diff);
            self.row_builds += 1;
            self.diff_key = Some((key, expanded.clone()));
        }
        let key = file_order::key(st, target);
        if self.list_key.as_ref() != Some(&key) {
            self.order = file_order::file_order(st, target);
            self.list_builds += 1;
            self.list_key = Some(key);
        }
    }
}

fn diff_key(st: &AppState, diff: &FileDiff) -> DiffKey {
    let mode = st.settings.diff_mode;
    DiffKey {
        generation: (st.session.diff.generation, st.session.diff.shown_gen),
        hunks: diff.hunks.as_ptr() as usize,
        whole: whole_file::shown(diff, mode).map(|w| w as *const WholeFile as usize),
        mode,
        layout: st.settings.diff_layout,
        lfs_summary: lfs_notes::summary(st).is_some(),
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
