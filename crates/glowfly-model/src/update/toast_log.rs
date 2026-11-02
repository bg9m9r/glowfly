//! Every toast leaves a line in the activity log, so one that expired, was closed or was dropped
//! by the toast limit can still be read there. A line the same update logged stands for one
//! toast of its outcome (a finished job's run, a failure the toast announces), the one with its
//! text first; every other toast is logged. A toast updated in place (zoom's) was logged when it
//! first showed.

use crate::state::AppState;
use crate::state::logs::{EntryId, headline};

/// Logs the toasts shown since `toast_mark` that no line logged since `entry_mark` stands for.
pub(super) fn record(st: &mut AppState, toast_mark: u64, entry_mark: EntryId) {
    let mut lines: Vec<(bool, String)> = st
        .log
        .iter()
        .chain(st.tabs.iter().flat_map(|t| t.log.iter()))
        .filter(|e| e.id > entry_mark && e.earlier.is_none())
        .map(|e| (e.ok, e.text.clone()))
        .collect();
    let fresh: Vec<(bool, String)> = st.toasts.iter().filter(|t| t.id > toast_mark).map(|t| (t.ok, t.text.clone())).collect();
    // Lines whose text a toast shows go to that toast first.
    let mut unpaired = Vec::new();
    for (ok, text) in fresh {
        match lines.iter().position(|(o, l)| *o == ok && (*l == text || headline(l) == text)) {
            Some(i) => {
                lines.remove(i);
            }
            None => unpaired.push((ok, text)),
        }
    }
    for (ok, text) in unpaired {
        match lines.iter().position(|(o, _)| *o == ok) {
            Some(i) => {
                lines.remove(i);
            }
            None => st.log_quiet(ok, text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::state;
    use std::time::Instant;

    fn texts(st: &AppState) -> Vec<(bool, &str)> {
        st.log.iter().map(|e| (e.ok, e.text.as_str())).collect()
    }

    #[test]
    fn each_line_stands_for_one_toast() {
        let (mut st, now) = (state(), Instant::now());
        st.log.clear();
        // Two successes and nothing logged: both get a line.
        let (toasts, entries) = (st.toast_seq, EntryId::next());
        st.toast(true, "Copied", now);
        st.toast(true, "Saved", now);
        record(&mut st, toasts, entries);
        assert_eq!(texts(&st), [(true, "Copied"), (true, "Saved")]);

        // A logged failure stands for its own toast; a second failure toasted beside it is logged.
        st.log.clear();
        let (toasts, entries) = (st.toast_seq, EntryId::next());
        st.log(false, "Push failed: offline\nfatal: unable to access");
        st.toast(false, "Lock file left behind", now);
        st.toast(false, "Push failed: offline", now);
        record(&mut st, toasts, entries);
        assert_eq!(texts(&st), [(false, "Push failed: offline\nfatal: unable to access"), (false, "Lock file left behind")]);

        // A finished job's run line stands for its differently worded toast.
        st.log.clear();
        let (toasts, entries) = (st.toast_seq, EntryId::next());
        st.log(true, "Everything up-to-date");
        st.toast(true, "Push succeeded", now);
        record(&mut st, toasts, entries);
        assert_eq!(texts(&st), [(true, "Everything up-to-date")]);
    }
}
