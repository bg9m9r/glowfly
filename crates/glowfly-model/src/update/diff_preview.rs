//! The binary diff's sizes and image previews: asked for when a binary diff is shown
//! (`DiffEffect::BinarySides`), kept through a reload of the same file until the new sides
//! arrive, ignored when they come back for a diff that is no longer on screen, and asked for
//! again when a tab comes back with them still loading. An SVG diffed as text asks for them
//! while its Text / Preview switch says Preview (`set_svg_preview`).

use crate::effect::DiffEffect;
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::diff_preview::DiffPreview;
use crate::state::error_words::typed;
use glowfly_core::error::ReadFailed;
use glowfly_core::repo::{BinarySides, DiffTarget};
use std::sync::Arc;

/// A diff was shown (opened or reloaded): a binary one without an LFS summary, or an SVG shown
/// drawn, asks for its sides, keeping the previous ones on screen when it is the same file; any
/// other drops them.
pub fn diff_shown(st: &mut AppState, fx: &mut Vec<Effect>) {
    let drawn = crate::state::diff_preview::drawn(st).then(|| st.session.diff.view.as_ref().map(|(t, _)| t.clone())).flatten();
    let Some(target) = drawn.filter(|_| crate::state::lfs_notes::summary(st).is_none()) else {
        st.session.diff_preview = None;
        return;
    };
    let kept = st.session.diff_preview.take().filter(|p| p.target == target).and_then(|p| p.sides);
    st.session.diff_preview = Some(DiffPreview { target, generation: st.session.diff.shown_gen, sides: kept });
    request(st, fx);
}

/// The header's Text / Preview switch for SVG files, remembered in the settings: the diff on
/// screen, when it is an SVG, asks for its sides (or drops them).
pub(super) fn set_svg_preview(st: &mut AppState, on: bool, fx: &mut Vec<Effect>) {
    if st.settings.diff_svg_preview == on {
        return;
    }
    st.settings.diff_svg_preview = on;
    fx.extend(super::prefs::save_settings(st));
    // The text selected and the change stepped to were on rows no longer (or not yet) shown.
    st.view.diff_text = None;
    st.cues.diff_cursor = None;
    if crate::state::diff_preview::svg_switch(st).is_some() {
        diff_shown(st, fx);
    }
}

/// Preferences edited the settings in place: an SVG on screen whose Preview setting changed
/// there asks for its sides, or drops them.
pub(super) fn settings_edited(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(on) = crate::state::diff_preview::svg_switch(st) else { return };
    if on != st.session.diff_preview.is_some() {
        diff_shown(st, fx);
    }
}

fn request(st: &AppState, fx: &mut Vec<Effect>) {
    if let (Some(workdir), Some(p)) = (st.workdir(), st.session.diff_preview.as_ref()) {
        fx.push(Effect::Diff(DiffEffect::BinarySides { workdir, target: p.target.clone(), generation: p.generation }));
    }
}

/// The sides read for the diff shown at `generation`: kept when that diff is still the one the
/// preview is for. A failure is stored too, so the body falls back to its plain binary state.
pub(super) fn loaded(st: &mut AppState, target: DiffTarget, generation: u64, result: Result<Arc<BinarySides>, ReadFailed>) {
    let t = st.settings.lang();
    let shown = st.session.diff.view.as_ref().is_some_and(|(t, _)| *t == target);
    let Some(p) = st.session.diff_preview.as_mut().filter(|p| p.target == target && shown && generation == st.session.diff.shown_gen)
    else {
        return;
    };
    if let Err(e) = &result {
        tracing::debug!("binary sides of {}: {e}", target.path());
    }
    p.sides = Some(result.map_err(|e| typed::read(t, &e)));
}

/// A tab came back: its preview's result was dropped while it was parked, so a preview still
/// loading for the diff on screen asks again.
pub fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let generation = st.session.diff.shown_gen;
    let on_screen = st.session.diff.view.as_ref().map(|(t, _)| t.clone());
    let Some(p) = st.session.diff_preview.as_mut().filter(|p| p.sides.is_none() && on_screen.as_ref() == Some(&p.target)) else {
        return;
    };
    p.generation = generation;
    request(st, fx);
}
