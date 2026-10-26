//! The binary diff's sizes and image previews: asked for when a binary diff is shown
//! (`Effect::LoadBinarySides`), kept through a reload of the same file until the new sides
//! arrive, ignored when they come back for a diff that is no longer on screen, and asked for
//! again when a tab comes back with them still loading.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::diff_preview::DiffPreview;
use glowfly_core::repo::{BinarySides, DiffTarget};
use std::sync::Arc;

/// A diff was shown (opened or reloaded): a binary one without an LFS summary asks for its
/// sides, keeping the previous ones on screen when it is the same file; any other drops them.
pub(super) fn diff_shown(st: &mut AppState, fx: &mut Vec<Effect>) {
    let binary = st.diff.as_ref().filter(|(_, d)| d.binary).map(|(t, _)| t.clone());
    let Some(target) = binary.filter(|_| crate::state::lfs_notes::summary(st).is_none()) else {
        st.session.diff_preview = None;
        return;
    };
    let kept = st.session.diff_preview.take().filter(|p| p.target == target).and_then(|p| p.sides);
    st.session.diff_preview = Some(DiffPreview { target, generation: st.diff_shown_gen, sides: kept });
    request(st, fx);
}

fn request(st: &AppState, fx: &mut Vec<Effect>) {
    if let (Some(workdir), Some(p)) = (st.workdir(), st.session.diff_preview.as_ref()) {
        fx.push(Effect::LoadBinarySides { workdir, target: p.target.clone(), generation: p.generation });
    }
}

/// The sides read for the diff shown at `generation`: kept when that diff is still the one the
/// preview is for. A failure is stored too, so the body falls back to its plain binary state.
pub(super) fn loaded(st: &mut AppState, target: DiffTarget, generation: u64, result: Result<Arc<BinarySides>, String>) {
    let shown = st.diff.as_ref().is_some_and(|(t, _)| *t == target);
    let Some(p) = st.session.diff_preview.as_mut().filter(|p| p.target == target && shown && generation == st.diff_shown_gen) else {
        return;
    };
    if let Err(e) = &result {
        tracing::debug!("binary sides of {}: {e}", target.path());
    }
    p.sides = Some(result);
}

/// A tab came back: its preview's result was dropped while it was parked, so a preview still
/// loading for the diff on screen asks again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let generation = st.diff_shown_gen;
    let on_screen = st.diff.as_ref().map(|(t, _)| t.clone());
    let Some(p) = st.session.diff_preview.as_mut().filter(|p| p.sides.is_none() && on_screen.as_ref() == Some(&p.target)) else {
        return;
    };
    p.generation = generation;
    request(st, fx);
}
