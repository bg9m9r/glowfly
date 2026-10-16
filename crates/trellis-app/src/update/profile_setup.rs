//! What follows a profile switch besides the identity: the app catching up with the
//! preferences and connected accounts it loaded (`Settings::swap_profile_setup`), and keeping
//! a shared sign-in while any profile still lists its account.

use crate::effect::Effect;
use crate::state::{AppState, ForgeState};
use std::time::Instant;
use trellis_core::forge::Account;
use trellis_core::settings::SetupSwap;

/// The switch to the now-current profile loaded these changes. Views read the settings every
/// frame, so only what keeps state of its own needs telling.
pub(super) fn switched(st: &mut AppState, swap: SetupSwap, now: Instant, fx: &mut Vec<Effect>) {
    if swap.prefs {
        super::prefs::sync_live_settings(st, fx);
        // The diff's scroll offset is in pixels: keep the same top row if the font size or
        // wrapping changed.
        if st.diff.is_some() {
            st.view.diff_scroll_to_row = Some(st.view.diff_top_row);
        }
        // The commit limit or branch visibility may have changed.
        if st.snap.is_some() {
            super::repo::refresh(st, fx);
        }
    }
    if swap.connections {
        reset_forge(st, now, fx);
    }
}

/// The connected accounts changed: sign in again and reload pull requests and issues in the
/// active tab; parked tabs detect their service again when selected.
fn reset_forge(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(old) = st.forge.take() {
        st.forge = Some(ForgeState::new(old.host, old.remote, old.tracker));
    }
    // A self-hosted server is recognized only through a connection.
    if let Some(snap) = st.snap.take() {
        super::forge::detect(st, &snap);
        st.snap = Some(snap);
    }
    super::forge::refresh(st, true, false, now, fx);
    for tab in &mut st.tabs {
        if let Some(parked) = tab.parked.as_mut() {
            parked.forge = None;
        }
    }
    if let Some(provider) = st.prefs.as_ref().map(|p| p.int_provider) {
        super::prefs::select_integration(st, provider);
    }
}

/// Deletes the stored sign-ins of `accounts` that no profile lists any more.
pub(super) fn forget_tokens(st: &AppState, accounts: impl IntoIterator<Item = Account>, fx: &mut Vec<Effect>) {
    for a in accounts {
        if !st.settings.account_in_use(&a) {
            fx.push(Effect::DeleteToken(a));
        }
    }
}
