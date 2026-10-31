//! A settings file Glowfly couldn't read: saving is held back (`save_settings` makes no save
//! while `st.settings_file.unreadable` is set), the banner says so and offers Try again, which
//! reads the file again (`PrefsEffect::ReadSettings`), and Open settings folder. Once a try
//! reads it, its settings replace the defaults in use and saving goes on; the reload has kept
//! a copy as `.bak` first.

use crate::effect::{Effect, OsEffect, PrefsEffect};
use crate::state::AppState;
use glowfly_core::app_update::Version;
use glowfly_core::settings::{LoadProblem, Settings, SetupSwap};
use std::time::Instant;

/// The settings file couldn't be read at startup: in the activity log, with a toast unless
/// saving now waits for it (then the banner says so, and stays until it reads).
pub(super) fn at_start(st: &mut AppState, problem: LoadProblem, now: Instant) {
    tracing::warn!("settings: {}", problem.message());
    match problem.locked {
        Some(_) => {
            let t = st.settings.lang();
            st.log_quiet(false, t.trf("settings_file.unreadable.log", &[("detail", &problem.detail)]));
            // Usually already held by the shell, before the startup steps.
            st.settings_file.hold(Some(&problem));
            // What the startup steps made of the defaults is where the user's edits start: a
            // later read tells whether anything they changed is dropped.
            st.settings_file.saved = serde_json::to_string(&st.settings).unwrap_or_default();
        }
        None => {
            let text = problem.message();
            st.log(false, text.clone());
            st.toast(false, text, now);
        }
    }
}

/// Try again: reads the file again, once at a time.
pub(super) fn retry(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(u) = st.settings_file.unreadable.as_mut() else { return };
    if !u.reading {
        u.reading = true;
        fx.push(Effect::Prefs(PrefsEffect::ReadSettings));
    }
}

/// Open settings folder: the folder holding the file, in the file manager.
pub(super) fn open_folder(st: &AppState, fx: &mut Vec<Effect>) {
    if let Some(dir) = st.settings_file.unreadable.as_ref().and_then(|u| u.file.parent()) {
        fx.push(Effect::Os(OsEffect::OpenFolder(dir.to_owned())));
    }
}

/// A try at reading the file finished. Still unreadable: the banner stays, with the new error.
/// Read: its settings are the ones in use from now on, and saving goes on.
pub(super) fn reread(st: &mut AppState, settings: Settings, problem: Option<LoadProblem>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(u) = st.settings_file.unreadable.as_mut() else { return };
    u.reading = false;
    let t = st.settings.lang();
    if let Some(LoadProblem { locked: Some(_), detail, .. }) = &problem {
        u.detail = detail.clone();
        // Announced by a toast with Details.
        st.log(false, t.trf("settings_file.still.log", &[("detail", detail)]));
        return;
    }
    st.settings_file.unreadable = None;
    let git_changed = settings.git_program != st.settings.git_program;
    // Edits made while saving was held back are dropped for the file's settings.
    let edited = serde_json::to_string(&st.settings).ok().as_deref() != Some(st.settings_file.saved.as_str());
    st.settings_file.saved = serde_json::to_string(&settings).unwrap_or_default();
    // The release notes recorded seen at this start stay seen, or they would open again.
    let notes = newer_notes(st.settings.updates.notes_seen.take(), settings.updates.notes_seen.clone());
    let notes_newer = notes != settings.updates.notes_seen;
    st.settings = settings;
    st.settings.updates.notes_seen = notes;
    // Built from the settings only at startup; everything else reads them every frame or is
    // brought up to date by `switched` below.
    st.terminal.open = st.settings.terminal.panel_open;
    let (keymap, key_problems) = crate::keymap::Keymap::from_overrides(&st.settings.keybindings);
    st.keymap = keymap;
    for p in key_problems {
        st.log(false, p);
    }
    // The language may have changed with the settings.
    let t = st.settings.lang();
    match problem {
        // Damaged but moved aside: said like at startup.
        Some(problem) => at_start(st, problem, now),
        None => {
            let text = if edited { t.tr("settings_file.read_dropped.toast") } else { t.tr("settings_file.read.toast") };
            st.log(true, text);
            st.toast(true, text, now);
        }
    }
    if notes_newer {
        fx.extend(super::prefs::save_settings(st));
    }
    super::profile_setup::switched(st, SetupSwap { prefs: true, connections: true }, now, fx);
    // The git that runs was set from the defaults at startup: put the file's into effect.
    if git_changed {
        let program = st.settings.git_program.as_deref().map(std::path::PathBuf::from);
        super::git_env::use_program(st, program, fx);
    }
}

/// The later of two "release notes seen" versions; one that doesn't parse counts as none.
fn newer_notes(ours: Option<String>, file: Option<String>) -> Option<String> {
    let parsed = |v: &Option<String>| v.as_deref().and_then(Version::parse);
    match (parsed(&ours), parsed(&file)) {
        (Some(a), Some(b)) if a > b => ours,
        (Some(_), None) => ours,
        _ => file,
    }
}
