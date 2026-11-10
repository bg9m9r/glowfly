//! What the runner reports about saving recent.txt and tabs.txt (runner/platform/state_files.rs):
//! a copy kept of a file with lines it couldn't read, a list read only late (the start read
//! failed) whose entries now join this session's, and a save refused because the file still
//! can't be read.

use crate::effect::{Effect, SaveEffect};
use crate::msg::{SaveNotes, StateFile};
use crate::state::AppState;
use crate::state::repo_lists::RECENT_LIMIT;
use glowfly_core::{native_path, paths};
use std::time::Instant;

/// A save of `file` went through and did more than write it: logs the copy it kept (without a
/// toast), and merges a late-read recent list into the Recent list, saves the merged list (the
/// runner merges late entries into every save until one holds them, so without this one a removed
/// entry would come back) and checks which of its folders are gone. Tabs read late come back at the next start; this session only logs them.
pub(super) fn saved(st: &mut AppState, file: StateFile, notes: SaveNotes, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    if let Some((backup, skipped)) = notes.kept {
        let path = native_path::shown(&backup);
        let text = match file {
            StateFile::Recent => t.trn("repos.recent_file.kept.log", skipped, &[("path", &path)]),
            StateFile::Tabs => t.trn("tabs.saved_file.kept.log", skipped, &[("path", &path)]),
        };
        // Quiet: the lines were already gone at start, and nothing more is lost.
        st.log_quiet(false, text);
    }
    let Some(late) = notes.read_late else { return };
    match file {
        StateFile::Recent => {
            for p in late {
                if !st.recent.iter().any(|r| paths::same_spelling(r, &p)) {
                    st.recent.push(p);
                }
            }
            st.recent.truncate(RECENT_LIMIT);
            fx.push(Effect::Save(SaveEffect::Recent(st.recent.clone())));
            st.log(true, t.tr("repos.recent_file.read_late.log"));
            super::recent::check_folders(st, fx);
        }
        StateFile::Tabs => st.log(true, t.tr("tabs.saved_file.read_late.log")),
    }
}

/// A save of `file` was refused because it still can't be read: a toast and a log line saying
/// that changes aren't saved (or, for Clear recent, that the list comes back at the next start).
pub(super) fn blocked(st: &mut AppState, file: StateFile, clearing: bool, detail: &str, now: Instant) {
    let t = st.settings.lang();
    let args: &[(&str, &dyn std::fmt::Display)] = &[("detail", &detail)];
    let text = match (file, clearing) {
        (StateFile::Recent, true) => t.trf("repos.clear_recent.blocked.toast", args),
        (StateFile::Recent, false) => t.trf("repos.recent_file.blocked.toast", args),
        (StateFile::Tabs, _) => t.trf("tabs.saved_file.blocked.toast", args),
    };
    st.log(false, text.clone());
    st.toast(false, text, now);
}
