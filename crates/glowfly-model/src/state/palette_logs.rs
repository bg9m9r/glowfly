//! The palette's log entries: view the activity, error and performance logs, open the logs
//! folder, export every log as one bundle, and turn extended logging on or off. Available with or without a repository.

use super::{Entry, PaletteCommand, entry};
use crate::logs::{LogKind, LogsMsg};
use crate::state::AppState;

pub(super) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    let logs = PaletteCommand::Logs;
    out.push(entry(logs(LogsMsg::Show(LogKind::Activity)), "View activity log", "activity output messages history panel log"));
    out.push(entry(
        logs(LogsMsg::Show(LogKind::Errors)),
        "View error log",
        "errors warnings crash panic problems diagnostics support file log",
    ));
    out.push(entry(
        logs(LogsMsg::Show(LogKind::Performance)),
        "View performance log",
        "performance slow timings speed diagnostics support file log",
    ));
    out.push(entry(logs(LogsMsg::OpenFolder), "Open logs folder", "logs directory folder explorer finder files support diagnostics"));
    out.push(entry(logs(LogsMsg::Export), "Export logs…", "export save bundle archive bug report support diagnostics logs file"));
    let on = st.settings.extended_logging;
    let label = if on { "Turn off extended logging" } else { "Turn on extended logging" };
    out.push(entry(logs(LogsMsg::SetExtended(!on)), label, "extended verbose debug logging git commands timings diagnostics"));
}
