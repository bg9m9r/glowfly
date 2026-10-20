//! Pinned branch colours and the palette's hue count: a pin from a branch menu's Color submenu
//! is saved per repository and recolours the graph at once, without a reload. Every load, a
//! Preferences edit or profile switch and a tab coming back from the background bring the
//! snapshot's colouring in line with the settings.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::branches::MAIN_USES_ACCENT;
use trellis_core::repo::Snapshot;
use trellis_core::settings::Settings;

/// Pins the branch key `key` to hue `slot` in the active repository, or with None lets it pick
/// its own again. The main branch is refused (the menu disables it; a late message logs why),
/// and so is a slot past the active palette's hues.
pub(super) fn pin(st: &mut AppState, key: &str, slot: Option<u8>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if snap.graph.owners.mainline.as_deref() == Some(key) {
        st.log(false, MAIN_USES_ACCENT);
        return;
    }
    if slot.is_some_and(|s| s >= st.settings.branch_palette.slots()) {
        return;
    }
    let real = snap.real_workdir.clone();
    if st.settings.set_branch_pin(&real, key, slot) {
        sync(st);
        fx.push(super::prefs::save_settings(st));
    }
}

/// Recolours the active snapshot when its colouring differs from the settings'.
pub(super) fn sync(st: &mut AppState) {
    if let Some(snap) = st.snap.as_mut() {
        apply(&st.settings, snap);
    }
}

/// Gives `snap` (installed or about to be) the colouring the settings hold for its repository.
pub(super) fn apply(settings: &Settings, snap: &mut Snapshot) {
    let want = settings.colouring_for(&snap.real_workdir);
    if snap.graph.colouring != want {
        snap.set_colouring(want);
    }
}
