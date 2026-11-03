//! Help > Getting started (state/getting_started.rs): opening and closing its page, a card's
//! Show me, listing the finished cards, and hiding the page from the Help menu and the welcome.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::onboarding::getting_started::{Card, GettingStarted, HIDDEN_ID, ShowMe};
use crate::state::palette::Palette;
use std::time::Instant;

/// Opens the page over whatever the central area shows; the welcome, when open, is finished
/// first (its Start step links here) and the tour ends.
pub(super) fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.onboarding.welcome.is_some() {
        super::finish(st, fx);
    }
    super::close_tour(st);
    st.dashboard.open = false;
    if st.onboarding.getting_started.is_none() {
        st.onboarding.getting_started = Some(GettingStarted::default());
    }
}

/// A card's Show me: the page closes, then the card's feature opens. A card that can't show
/// itself now (its button is disabled, with the reason) leaves the page as it is.
pub(super) fn show_me(st: &mut AppState, card: Card, now: Instant, fx: &mut Vec<Effect>) {
    let Ok(action) = card.show_me(st) else { return };
    st.onboarding.getting_started = None;
    match action {
        ShowMe::Send(msg) => fx.extend(super::super::update_sealed(st, *msg, now)),
        ShowMe::Search(text) => {
            if st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing() {
                st.palette = Some(Palette { query: text.to_owned(), ..Palette::default() });
            }
        }
        ShowMe::Nothing => {}
    }
}

pub(super) fn show_finished(st: &mut AppState, on: bool) {
    if let Some(p) = st.onboarding.getting_started.as_mut() {
        p.show_finished = on;
    }
}

/// Hides the page from the Help menu and the welcome (the palette still lists it), or shows
/// it there again; saved with the tips seen, so Show tips again brings it back too.
pub(super) fn hide(st: &mut AppState, hide: bool, now: Instant, fx: &mut Vec<Effect>) {
    let seen = &mut st.settings.hints_seen;
    let changed = if hide { seen.insert(HIDDEN_ID.to_owned()) } else { seen.remove(HIDDEN_ID) };
    if !changed {
        return;
    }
    fx.extend(super::super::prefs::save_settings(st));
    let t = st.settings.lang();
    let text = if hide { t.tr("getting_started.hidden.toast") } else { t.tr("getting_started.shown.toast") };
    st.toast(true, text, now);
}
