//! Repository colours (Vivid colour): the tab menu's and the repository list's Color submenus
//! give a working folder a colour or take it away. Saved per folder in the settings, which are
//! written only when the choice changes.

use crate::effect::Effect;
use crate::state::AppState;
use glowfly_core::paths::same_spelling;
use glowfly_core::settings::RepoColour;
use std::path::Path;

/// Gives the repository at `path` the colour `colour` (None takes it away). The colour is saved
/// under the repository's settings key (where an open tab found it really is), and an entry left
/// under the spelling the menu sent goes, so the list and the tab agree and a later load keeps it.
pub(super) fn set(st: &mut AppState, path: &Path, colour: Option<RepoColour>, fx: &mut Vec<Effect>) {
    let key = st.repo_key(path);
    let mut changed = st.settings.set_repo_colour(&key, colour);
    if !same_spelling(&key, path) {
        changed |= st.settings.set_repo_colour(path, None);
    }
    if changed {
        fx.extend(super::prefs::save_settings(st));
    }
}
