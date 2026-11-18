//! The palette's entries for the open diff's unchanged runs: Show all (while some are closed)
//! and Fold all (while some are open), in the hunk view and the whole-file view alike. None with
//! Ignore whitespace on in the hunk view, where the runs can't be shown.

use super::{Entry, PaletteCommand as C, entry};
use crate::state::AppState;
use crate::state::diff_runs::counts;

pub(super) fn diff_entries(st: &AppState, out: &mut Vec<Entry>) {
    let Some(c) = counts(st).filter(|c| !c.ws_hidden) else { return };
    let t = st.settings.lang();
    if c.closed > 0 {
        out.push(entry(
            C::UnchangedLines(true),
            t.tr("palette.diff.expand_all"),
            "expand all unchanged lines context gaps unfold whole diff",
        ));
    }
    if c.open > 0 {
        out.push(entry(
            C::UnchangedLines(false),
            t.tr("palette.diff.collapse_all"),
            "collapse all unchanged lines context gaps fold hide diff",
        ));
    }
}
