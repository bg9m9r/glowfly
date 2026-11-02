//! Every toast leaves a line in the activity log, so one that expired, was closed or was dropped
//! by the toast limit can still be read there. A toast from an update that logged a line of the
//! same outcome already (a finished job's run, a failure the toast announces) is that line's
//! toast and isn't logged again; a toast updated in place (zoom's) was logged when it first
//! showed.

use crate::state::AppState;
use crate::state::logs::EntryId;

/// Logs the toasts shown since `toast_mark` when no entry logged since `entry_mark` has their
/// outcome.
pub(super) fn record(st: &mut AppState, toast_mark: u64, entry_mark: EntryId) {
    let fresh: Vec<(bool, String)> = st.toasts.iter().filter(|t| t.id > toast_mark).map(|t| (t.ok, t.text.clone())).collect();
    for (ok, text) in fresh {
        let logged =
            st.log.iter().chain(st.tabs.iter().flat_map(|t| t.log.iter())).any(|e| e.id > entry_mark && e.ok == ok && e.earlier.is_none());
        if !logged {
            st.log_quiet(ok, text);
        }
    }
}
