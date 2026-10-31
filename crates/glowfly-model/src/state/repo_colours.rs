//! Which repository colour (Vivid colour) shows where: a tab's, the active tab's (the window's
//! top line) and a listed folder's. Colours are saved per working folder in the settings
//! (`Settings::repo_colour_for`); a tab looks its colour up where it really is on disk once
//! loaded, and a listed path takes the colour of a tab showing it under another spelling. A
//! repository without a colour of its own takes its group's (`repo_groups`).

use super::{AppState, Tab};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoColour;
use std::path::Path;

/// The folder a tab's colour is saved under: where it really is once loaded, else its path
/// (None for a new tab).
pub fn tab_path(tab: &Tab) -> Option<&Path> {
    tab.real.as_deref().or(tab.path.as_deref())
}

/// The colour of the repository at `p`: its own, else its group's.
fn colour_at(st: &AppState, p: &Path) -> Option<RepoColour> {
    st.settings.repo_colour_for(p).or_else(|| super::repo_groups::group_hue(st, p))
}

/// The colour of tab `i`, if it has one.
pub fn tab_colour(st: &AppState, i: usize) -> Option<RepoColour> {
    if !super::repo_groups::any_colour(&st.settings) {
        return None;
    }
    let tab = st.tabs.get(i)?;
    // The group is found by the path it was opened by too, where the favorite is listed.
    tab_path(tab).and_then(|p| colour_at(st, p)).or_else(|| tab.path.as_deref().and_then(|p| super::repo_groups::group_hue(st, p)))
}

/// The active tab's colour: the window's top line.
pub fn active_colour(st: &AppState) -> Option<RepoColour> {
    tab_colour(st, st.active_tab)
}

/// The colour of a listed folder: its own or its group's, else that of a tab showing it.
pub fn listed_colour(st: &AppState, path: &Path) -> Option<RepoColour> {
    if !super::repo_groups::any_colour(&st.settings) {
        return None;
    }
    colour_at(st, path).or_else(|| st.tab_showing(path).and_then(|i| tab_colour(st, i)))
}

/// The hue's name in the Color submenu.
pub fn hue_name(t: Lang, c: RepoColour) -> &'static str {
    match c {
        RepoColour::Rose => t.tr("repo_colour.hue.rose"),
        RepoColour::Fern => t.tr("repo_colour.hue.fern"),
        RepoColour::Mint => t.tr("repo_colour.hue.mint"),
        RepoColour::Teal => t.tr("repo_colour.hue.teal"),
        RepoColour::Sky => t.tr("repo_colour.hue.sky"),
        RepoColour::Slate => t.tr("repo_colour.hue.slate"),
        RepoColour::Iris => t.tr("repo_colour.hue.iris"),
        RepoColour::Plum => t.tr("repo_colour.hue.plum"),
    }
}
