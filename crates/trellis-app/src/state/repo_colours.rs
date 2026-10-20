//! Which repository colour (Vivid colour) shows where: a tab's, the active tab's (the window's
//! top line) and a listed folder's. Colours are saved per working folder in the settings
//! (`Settings::repo_colour_for`); a tab looks its colour up where it really is on disk once
//! loaded, and a listed path takes the colour of a tab showing it under another spelling.

use super::{AppState, Tab};
use std::path::Path;
use trellis_core::i18n::Lang;
use trellis_core::settings::RepoColour;

/// The folder a tab's colour is saved under: where it really is once loaded, else its path
/// (None for a new tab).
pub fn tab_path(tab: &Tab) -> Option<&Path> {
    tab.real.as_deref().or(tab.path.as_deref())
}

/// The colour of tab `i`, if it has one.
pub fn tab_colour(st: &AppState, i: usize) -> Option<RepoColour> {
    if st.settings.repo_colours.is_empty() {
        return None;
    }
    st.tabs.get(i).and_then(tab_path).and_then(|p| st.settings.repo_colour_for(p))
}

/// The active tab's colour: the window's top line.
pub fn active_colour(st: &AppState) -> Option<RepoColour> {
    tab_colour(st, st.active_tab)
}

/// The colour of a listed folder: its own, else that of a tab showing it.
pub fn listed_colour(st: &AppState, path: &Path) -> Option<RepoColour> {
    if st.settings.repo_colours.is_empty() {
        return None;
    }
    st.settings.repo_colour_for(path).or_else(|| st.tab_showing(path).and_then(|i| tab_colour(st, i)))
}

/// The hue's name in the Color submenu.
pub fn hue_name(t: Lang, c: RepoColour) -> &'static str {
    match c {
        RepoColour::Rose => t.tr("repo_colour.hue.rose"),
        RepoColour::Ochre => t.tr("repo_colour.hue.ochre"),
        RepoColour::Fern => t.tr("repo_colour.hue.fern"),
        RepoColour::Teal => t.tr("repo_colour.hue.teal"),
        RepoColour::Sky => t.tr("repo_colour.hue.sky"),
        RepoColour::Slate => t.tr("repo_colour.hue.slate"),
        RepoColour::Iris => t.tr("repo_colour.hue.iris"),
        RepoColour::Plum => t.tr("repo_colour.hue.plum"),
    }
}
