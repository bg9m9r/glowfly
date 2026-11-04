//! Pinned branch colours and the palette's hue count: a pin from a branch menu's Color submenu
//! is saved per repository and recolours the graph at once, without a reload. Every load, a
//! Preferences edit or profile switch and a tab coming back from the background bring the
//! snapshot's colouring in line with the settings, and with the hue slots the shell reports as
//! passing for the trunk in the active palette.

use crate::effect::Effect;
use crate::state::AppState;
use glowfly_core::branch_colour::Colouring;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::Settings;

/// Pins the branch key `key` to hue `slot` in the active repository, or with None lets it pick
/// its own again (for the main branch, the trunk tone). A slot past the active palette's hues is
/// refused.
pub(super) fn pin(st: &mut AppState, key: &str, slot: Option<u8>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if slot.is_some_and(|s| s >= st.settings.colour_palette.slots()) {
        return;
    }
    let real = snap.real_workdir.clone();
    if st.settings.set_branch_pin(&real, key, slot) {
        sync(st);
        fx.extend(super::prefs::save_settings(st));
    }
}

/// Keeps the slots that pass for the trunk in the active palette (`mask`) and recolours the
/// active snapshot with them. A tab in the background takes them when it comes back.
pub(super) fn trunk_near(st: &mut AppState, mask: u16) {
    if st.view.trunk_near != mask {
        st.view.trunk_near = mask;
        sync(st);
    }
}

/// Recolours the active snapshot when its colouring differs from the settings'.
pub(super) fn sync(st: &mut AppState) {
    if let Some(snap) = st.snap.as_mut() {
        apply(&st.settings, st.view.trunk_near, snap);
    }
}

/// Gives `snap` (installed or about to be) the colouring the settings hold for its repository,
/// with the slots near the trunk (`trunk_near`).
pub(super) fn apply(settings: &Settings, trunk_near: u16, snap: &mut Snapshot) {
    let want = Colouring { trunk_near, ..settings.colouring_for(&snap.real_workdir) };
    if snap.graph.colouring != want {
        snap.set_colouring(want);
    }
}
