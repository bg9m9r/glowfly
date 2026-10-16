//! The logs: loading the earlier sessions' activity, writing new lines to the activity file,
//! the panel's tabs, the logs folder and extended logging. The runner's log worker does the
//! file work and stamps the times (runner/logs.rs).

use crate::effect::Effect;
use crate::logs::{EarlierLog, LogKind, LogsEffect, LogsMsg};
use crate::state::AppState;
use crate::state::logs::{LOG_LIMIT, LogEntry, LogFileView, Persist};
use trellis_core::logs::git_run_line;

pub(super) fn handle(st: &mut AppState, m: LogsMsg, fx: &mut Vec<Effect>) {
    match m {
        LogsMsg::Start => {
            fx.push(Effect::Logs(LogsEffect::LoadActivity));
            fx.push(Effect::Logs(LogsEffect::SetExtended(st.settings.extended_logging)));
        }
        LogsMsg::ActivityLoaded(Ok(earlier)) => loaded(st, earlier, fx),
        LogsMsg::ActivityLoaded(Err(e)) => {
            persistence_off(st);
            st.log_quiet(false, format!("Couldn't read the activity log from earlier sessions: {e}"));
        }
        LogsMsg::AppendFailed(e) => {
            persistence_off(st);
            st.log_quiet(false, format!("Couldn't save the activity log: {e}. New entries are kept for this session only."));
        }
        LogsMsg::Show(kind) => show(st, kind, fx),
        LogsMsg::FileRead { kind, generation, result } => {
            if generation == st.logs.file_gen
                && let Some(view) = st.logs.file.as_mut().filter(|v| v.kind == kind)
            {
                view.tail = Some(result);
            }
        }
        LogsMsg::OpenFolder => fx.push(Effect::Logs(LogsEffect::OpenFolder)),
        LogsMsg::SetExtended(on) => {
            if st.settings.extended_logging == on {
                return;
            }
            st.settings.extended_logging = on;
            fx.push(super::prefs::save_settings(st));
            fx.push(Effect::Logs(LogsEffect::SetExtended(on)));
            st.log_quiet(
                true,
                if on { "Extended logging is on: git commands and their timings are added here." } else { "Extended logging is off." },
            );
        }
        LogsMsg::GitRan(run) => {
            if st.settings.extended_logging {
                let ok = run.ok && !run.timed_out && run.code.is_some();
                st.log_quiet(ok, git_run_line(&run));
            }
        }
    }
}

/// Puts the earlier sessions' newest entries in front of this session's, as many as fit under
/// `LOG_LIMIT`, and starts writing new lines to the file.
fn loaded(st: &mut AppState, earlier: EarlierLog, fx: &mut Vec<Effect>) {
    st.logs.dir = Some(earlier.dir);
    st.logs.persist = Persist::On;
    // Cleared before the file was read: the earlier entries go too.
    if std::mem::take(&mut st.logs.clear_on_load) {
        fx.push(Effect::Logs(LogsEffect::ClearActivity));
        return;
    }
    let room = LOG_LIMIT.saturating_sub(st.log.len());
    let skip = earlier.entries.len().saturating_sub(room);
    let mut log: Vec<LogEntry> = earlier.entries.into_iter().skip(skip).map(LogEntry::earlier).collect();
    log.append(&mut st.log);
    st.log = log;
}

fn persistence_off(st: &mut AppState) {
    st.logs.persist = Persist::Off;
    st.logs.pending.clear();
}

/// Opens the panel on `kind`; a file tab reads its file again.
fn show(st: &mut AppState, kind: LogKind, fx: &mut Vec<Effect>) {
    st.view.show_log = true;
    st.logs.tab = kind;
    if kind == LogKind::Activity {
        return;
    }
    st.logs.file_gen += 1;
    st.logs.file = Some(LogFileView { kind, tail: None });
    fx.push(Effect::Logs(LogsEffect::ReadFile { kind, generation: st.logs.file_gen }));
}

/// The activity panel's Clear: empties the log, earlier sessions' entries too, and deletes the
/// file.
pub(super) fn clear(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.log.clear();
    st.logs.pending.clear();
    match st.logs.persist {
        Persist::On => fx.push(Effect::Logs(LogsEffect::ClearActivity)),
        Persist::Waiting => st.logs.clear_on_load = true,
        Persist::Off => {}
    }
}

/// Sends the lines waiting for the activity file, once it has loaded. Run at the end of every
/// `update`.
pub(super) fn flush(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(lines) = st.logs.take_append() {
        fx.push(Effect::Logs(LogsEffect::Append(lines)));
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "palette_logs_tests.rs"]
mod palette_tests;
