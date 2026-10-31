//! Graph column widths: a header divider drag or double-click stores the width and saves the
//! settings, only when the stored width changed.

use crate::effect::Effect;
use crate::state::AppState;
use glowfly_core::settings::ResizableColumn;

/// A divider drag ended at `width` (clamped and rounded when stored).
pub(super) fn resize(st: &mut AppState, col: ResizableColumn, width: f32, fx: &mut Vec<Effect>) {
    if st.settings.column_widths.set(col, width) {
        fx.extend(super::prefs::save_settings(st));
    }
}

/// A divider was double-clicked: back to the column's default width.
pub(super) fn reset(st: &mut AppState, col: ResizableColumn, fx: &mut Vec<Effect>) {
    if st.settings.column_widths.reset(col) {
        fx.extend(super::prefs::save_settings(st));
    }
}
