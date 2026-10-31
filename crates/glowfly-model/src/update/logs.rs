//! The logs: loading the earlier sessions' activity, writing new lines to the activity file,
//! the panel's tabs, the logs folder and extended logging. The runner's log worker does the
//! file work and stamps the times (runner/logs.rs). Finished jobs, open rows and fixes are in
//! update/logs_runs.rs; Copy entry and Copy log in update/logs_copy.rs; Export logs in
//! update/logs_export.rs.

use crate::effect::Effect;
use crate::logs::{EarlierLog, LogKind, LogsEffect, LogsMsg};
use crate::state::AppState;
use crate::state::logs::{LogFileView, Persist};
use glowfly_core::logs::{git_run_line, same_repo};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, m: LogsMsg, now: Instant, fx: &mut Vec<Effect>) {
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
                st.log_quiet_in(Some(&run.cwd), ok, git_run_line(&run));
            }
        }
        LogsMsg::Details { tab, entry } => super::logs_tabs::details(st, tab, entry, fx),
        LogsMsg::Fold(id) => super::logs_runs::fold(st, id),
        LogsMsg::Fix(id) => super::logs_runs::fix(st, id, false, now, fx),
        LogsMsg::AltFix(id) => super::logs_runs::fix(st, id, true, now, fx),
        LogsMsg::CopyEntry(id) => super::logs_copy::copy_entry(st, id, fx),
        LogsMsg::CopyLog => super::logs_copy::copy_log(st, now, fx),
        LogsMsg::Export => super::logs_export::export(st, fx),
        LogsMsg::ExportTo(dest) => super::logs_export::export_to(st, dest, fx),
        LogsMsg::Exported { dest, result } => super::logs_export::exported(st, dest, result, now),
        LogsMsg::ShowExported(dest) => super::logs_export::show_exported(dest, fx),
    }
}

/// Hands the earlier sessions' entries to their tabs, in front of this session's lines
/// (update/logs_tabs.rs), and starts writing new lines to the file.
fn loaded(st: &mut AppState, earlier: EarlierLog, fx: &mut Vec<Effect>) {
    st.logs.dir = Some(earlier.dir);
    st.logs.persist = Persist::On;
    let mut entries = earlier.entries;
    // Tabs cleared before the file was read: their earlier entries go too.
    for repo in std::mem::take(&mut st.logs.clear_on_load) {
        entries.retain(|e| !same_repo(e.repo.as_deref(), repo.as_deref()));
        fx.push(Effect::Logs(LogsEffect::ClearActivity { repo }));
    }
    super::logs_tabs::distribute(st, entries);
    super::logs_tabs::claim(st);
}

fn persistence_off(st: &mut AppState) {
    st.logs.persist = Persist::Off;
    st.logs.pending.clear();
}

/// Opens the panel on `kind`; a file tab reads its file again.
pub(super) fn show(st: &mut AppState, kind: LogKind, fx: &mut Vec<Effect>) {
    st.view.show_log = true;
    st.logs.tab = kind;
    if kind == LogKind::Activity {
        return;
    }
    st.logs.file_gen += 1;
    st.logs.file = Some(LogFileView { kind, tail: None });
    fx.push(Effect::Logs(LogsEffect::ReadFile { kind, generation: st.logs.file_gen }));
}

/// The activity panel's Clear: empties the active tab's log, earlier sessions' entries too,
/// and removes its repository's lines from the file. Other tabs keep theirs.
pub(super) fn clear(st: &mut AppState, fx: &mut Vec<Effect>) {
    let tag = st.tabs.get(st.active_tab).and_then(|t| t.path.clone());
    let cleared: std::collections::HashSet<_> = st.log.drain(..).map(|e| e.id).collect();
    st.logs.open.retain(|id| !cleared.contains(id));
    st.logs.fixes.retain(|(id, _)| !cleared.contains(id));
    st.logs.alt_fixes.retain(|(id, _)| !cleared.contains(id));
    st.logs.pending.retain(|l| !same_repo(l.repo.as_deref(), tag.as_deref()));
    st.logs.unclaimed.retain(|e| !same_repo(e.repo.as_deref(), tag.as_deref()));
    match st.logs.persist {
        Persist::On => fx.push(Effect::Logs(LogsEffect::ClearActivity { repo: tag })),
        Persist::Waiting => st.logs.clear_on_load.push(tag),
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
#[path = "tests/logs_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/palette_logs_tests.rs"]
mod palette_tests;
