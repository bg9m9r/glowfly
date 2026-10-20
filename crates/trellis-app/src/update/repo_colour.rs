//! Repository colours (Vivid colour): the tab menu's and the repository list's Color submenus
//! give a working folder a colour or take it away. Saved per folder in the settings, which are
//! written only when the choice changes.

use crate::effect::Effect;
use crate::state::AppState;
use std::path::Path;
use trellis_core::settings::RepoColour;

/// Gives the repository at `path` the colour `colour` (None takes it away).
pub(super) fn set(st: &mut AppState, path: &Path, colour: Option<RepoColour>, fx: &mut Vec<Effect>) {
    if st.settings.set_repo_colour(path, colour) {
        fx.push(super::prefs::save_settings(st));
    }
}
