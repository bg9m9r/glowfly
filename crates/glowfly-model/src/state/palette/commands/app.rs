//! The palette's app entries: commands that work with or without a repository (tabs, the
//! dashboard, Preferences, profiles, shortcuts, onboarding, updates, toolbar labels, layout,
//! themes, tools, logs).

use super::{Entry, PaletteCommand, entry, logs, with_detail};
use crate::state::AppState;
use glowfly_core::settings::ThemeChoice;

/// Commands that work with or without a repository: tabs, the dashboard, Preferences and
/// appearance.
pub(super) fn app_entries(st: &AppState, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    out.push(entry(C::OpenRepo, "Open repository…", "folder add"));
    crate::state::palette::tabs::tab_entries(st, out);
    out.push(entry(C::Dashboard, "Open dashboard", "pull requests issues reviews assigned across repositories overview inbox"));
    out.push(entry(C::OpenPrefs, "Open preferences", "settings options configure"));
    crate::state::palette::profiles::profile_entries(st, out);
    out.push(entry(C::KeyboardShortcuts, "Keyboard shortcuts", "keys keyboard bindings hotkeys help reference list"));
    crate::state::palette::onboarding::entries(st, out);
    crate::state::palette::updates::entries(st, out);
    let labels = st.settings.toolbar_labels;
    let label = if labels { "Hide toolbar labels" } else { "Show toolbar labels" };
    out.push(entry(C::ToolbarLabels(!labels), label, "toolbar buttons text captions icons compact"));
    crate::state::layout::palette_entries(st, out);
    let files =
        st.themes.catalog.files.iter().map(|f| (ThemeChoice::Custom(f.id.clone()), f.name.clone(), "appearance colour color custom"));
    let built_in = ThemeChoice::BUILT_IN.into_iter().map(|t| (t.clone(), t.palette_label(), "appearance colour color"));
    for (choice, name, words) in built_in.chain(files) {
        let current = st.settings.theme == choice;
        let e = entry(C::Theme(choice), format!("Theme: {name}"), words);
        out.push(with_detail(e, current.then(|| "current".to_owned())));
    }
    crate::state::palette::tools::palette_entries(st, out);
    logs::palette_entries(st, out);
}
