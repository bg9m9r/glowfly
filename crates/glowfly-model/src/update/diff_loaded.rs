//! A file diff's load result: shown (keeping selections and place on a reload of the same
//! text, landing a step or a code comment's line), a reload that finds nothing left, or a failure.

use super::repo::reload_shown_diff;
use crate::effect::Effect;
use crate::state::AppState;
use git2::Oid;
use glowfly_core::diff_rows::row_of_file_line;
use glowfly_core::lfs::pointer::LfsChange;
use glowfly_core::repo::{DiffTarget, FileDiff, GitlinkMove};

#[allow(clippy::too_many_arguments)]
pub(super) fn loaded(
    st: &mut AppState,
    target: DiffTarget,
    reload: bool,
    generation: u64,
    result: Result<FileDiff, String>,
    lfs: Option<LfsChange>,
    gitlink: Option<Box<GitlinkMove>>,
    raw: Option<Oid>,
    fx: &mut Vec<Effect>,
) {
    if generation != st.session.diff.generation {
        return; // superseded: another file was opened, the diff closed, or a newer reload sent
    }
    st.cues.diff_opening = None;
    // Any accepted result ends a pending step. A refresh while it loads asks for the step's
    // destination again (`diff::renew_step`) rather than reloading the old file, so a
    // superseding reload can't block it.
    let landing = st.cues.diff_landing.take();
    let line = st.cues.diff_open_line.take().filter(|(t, ..)| !reload && *t == target);
    match result {
        Ok(mut d) => {
            let landing = landing.filter(|(t, _)| !reload && *t == target).map(|(_, l)| l);
            // A stepped-to file with nothing to land on is passed over, never shown.
            if let Some(l) = landing
                && super::diff::skip_past(st, &target, &d, l, fx)
            {
                return;
            }
            // The line and text selections survive only a reload bringing back the same text (not just a new highlight).
            let open = st.session.diff.view.as_ref().filter(|(t, _)| *t == target);
            let same = reload && open.is_some_and(|(_, old)| old.binary == d.binary && old.hunks == d.hunks);
            let same_file = open.is_some();
            if !same {
                st.clear_diff_selection();
            } else if let Some(sel) = st.view.diff_text.as_mut() {
                sel.key.1 = generation; // keyed to the load it shows
            }
            // Folds opened in another file don't apply to this one.
            if !same_file {
                st.session.tab_view.diff_expanded.clear();
            }
            // A different file opens at the top (or its first change block, or the hunk kept
            // through a view mode switch, or the change a step lands on); a reload after a
            // hunk operation keeps its place, but its change rows may have moved.
            if !reload {
                let hunk = st.cues.diff_open_hunk.take();
                let row = match landing {
                    Some(l) => super::diff::landing_row(&d, &st.settings, l, &st.session.tab_view.diff_expanded),
                    None => {
                        let (s, ex) = (&st.settings, &st.session.tab_view.diff_expanded);
                        // A file opened from a code comment lands on its line.
                        line.and_then(|(_, side, n)| row_of_file_line(&d, s.diff_mode, s.diff_layout, ex, side, n))
                            .unwrap_or_else(|| super::diff::opening_row(&d, s, hunk, ex))
                    }
                };
                st.view.diff_scroll_to_row = Some(row);
                st.view.tab.diff_top_row = row;
                st.cues.diff_cursor = landing.map(|_| row);
            } else {
                st.cues.diff_cursor = None;
            }
            if reload {
                super::highlight::keep_finished(st.session.diff.view.as_ref(), &target, &mut d);
            }
            // A step landed: its file list shows the file once.
            if landing.is_some() {
                st.cues.file_reveal = Some((target.clone(), generation));
            }
            super::lfs::diff_loaded(st, &target, lfs);
            st.session.diff.gitlink = gitlink.map(|g| (target.clone(), g));
            st.session.diff.view = Some((target, std::sync::Arc::new(d)));
            st.session.diff.raw = raw;
            st.session.diff.shown_gen = generation;
            super::highlight::schedule(st, fx);
            super::diff_gaps::diff_shown(st, fx);
            super::diff_preview::diff_shown(st, fx);
        }
        // A reload that finds no changes left closes the diff, or follows a new file unstaged.
        Err(_) if reload => super::wip_list::emptied(st, target, fx),
        Err(e) => {
            st.cues.diff_lines.clear();
            st.cues.diff_open_hunk = None;
            st.log(false, e);
            // The diff still on screen was marked stale by this request: re-read it. Not
            // `reload_diff`, which would ask again for a file history entry that failed.
            reload_shown_diff(st, fx);
        }
    }
}
