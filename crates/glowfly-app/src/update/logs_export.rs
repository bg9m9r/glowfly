//! Export logs: ask where to save (a worker thread runs the dialog), then the log worker writes
//! every log as one bundle after the appends queued before it (runner/logs.rs), under a header
//! naming the versions and the platform for a bug report.

use crate::effect::{Effect, OsEffect};
use crate::logs::{LogsEffect, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_bar::VERSION;
use crate::state::toasts::ToastAction;
use glowfly_core::logs::{self, BundleReport};
use glowfly_core::{git_version, native_path};
use std::path::PathBuf;
use std::time::Instant;

/// Asks where to save the bundle, suggesting a name with the time in it.
pub(super) fn export(st: &AppState, fx: &mut Vec<Effect>) {
    fx.push(Effect::Logs(LogsEffect::PickExport { name: logs::bundle_name(st.view.now, st.view.utc_offset) }));
}

/// The save dialog's answer: writes the bundle there, or nothing when it was cancelled.
pub(super) fn export_to(st: &AppState, dest: Option<PathBuf>, fx: &mut Vec<Effect>) {
    if let Some(dest) = dest {
        fx.push(Effect::Logs(LogsEffect::Export { dest, header: header(st) }));
    }
}

/// The bundle's header, in fixed English for maintainers. It leaves out the logs folder (the
/// user's profile path); git's path is in, as in About's Copy version info.
pub(super) fn header(st: &AppState) -> Vec<(String, String)> {
    let git = match &st.git.info {
        None => "not checked yet".to_owned(),
        Some(info) => match (info.version, &info.path) {
            (None, _) => "not found".to_owned(),
            (Some(v), Some(path)) => format!("{} ({})", git_version::show(v), native_path::shown(path)),
            (Some(v), None) => git_version::show(v),
        },
    };
    let pairs = [
        ("Exported", logs::stamp(st.view.now, st.view.utc_offset)),
        ("Glowfly", VERSION.to_owned()),
        ("OS", logs::platform()),
        ("Git", git),
        ("Extended logging", if st.settings.extended_logging { "on" } else { "off" }.to_owned()),
        ("Language", st.settings.lang().code().to_owned()),
    ];
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// The log worker's answer: a toast that can show the file, or the failure (announced, so its
/// toast gets Details).
pub(super) fn exported(st: &mut AppState, dest: PathBuf, result: Result<BundleReport, String>, now: Instant) {
    match result {
        Ok(r) => {
            let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let files = if r.files == 1 { "1 file".to_owned() } else { format!("{} files", r.files) };
            let label = crate::state::files::show_in_folder(st.settings.lang()).to_owned();
            let action = ToastAction { label, msg: Box::new(Msg::Logs(LogsMsg::ShowExported(dest.clone()))), workdir: None };
            st.toast_action(true, format!("Exported the logs to {name} ({files})"), action, now);
            st.log_quiet(true, format!("Exported the logs to {}", native_path::shown(&dest)));
        }
        Err(e) => {
            let text = format!("Couldn't export the logs: {e}");
            st.toast(false, text.clone(), now);
            st.log(false, text);
        }
    }
}

/// The export toast's action.
pub(super) fn show_exported(dest: PathBuf, fx: &mut Vec<Effect>) {
    fx.push(Effect::Os(OsEffect::RevealPath(dest)));
}

#[cfg(test)]
#[path = "logs_export_tests.rs"]
mod tests;
