//! Syntax highlighting finished in the background: large diffs arrive with a pending highlight
//! (plain or provisional spans), `Effect::HighlightDiff` parses them on a worker thread (the
//! lines and the function context in the hunk headers), and `Msg::DiffHighlighted` attaches the
//! result if the same diff is still on screen.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use trellis_core::highlight::DiffHighlight;
use trellis_core::repo::{DiffTarget, FileDiff};

fn pending(d: &FileDiff) -> bool {
    d.highlight.as_ref().is_some_and(DiffHighlight::is_pending)
}

/// On a reload that brings back the same hunks: keeps the finished highlight on screen instead
/// of a pending one (a background result can land after the reload was requested, while the
/// diff it sent along as `previous` was still pending), if both were computed on the same basis
/// (syntax and old text: per-hunk spans never stand in for in-context ones, nor spans from an
/// older old text). Likewise for the whole file.
pub(super) fn keep_finished(shown: Option<&(DiffTarget, std::sync::Arc<FileDiff>)>, target: &DiffTarget, new: &mut FileDiff) {
    let Some((_, old)) = shown.filter(|(t, _)| t == target) else { return };
    adopt(old, new);
    if let (Some(ow), Some(nw)) = (old.whole.as_deref(), new.whole.as_deref_mut()) {
        adopt(&ow.diff, &mut nw.diff);
    }
}

fn adopt(old: &FileDiff, new: &mut FileDiff) {
    if let (Some(o), Some(n)) = (old.highlight.as_ref(), new.highlight.as_ref())
        && !o.is_pending()
        && n.is_pending()
        && o.same_basis(n)
        && old.hunks == new.hunks
    {
        new.highlight = Some(o.clone());
    }
}

/// Starts the background pass for the diff on screen if its hunks or whole file are pending.
pub(super) fn schedule(st: &AppState, fx: &mut Vec<Effect>) {
    let (Some(workdir), Some((target, d))) = (st.workdir(), st.diff.as_ref()) else { return };
    if pending(d) || d.whole.as_deref().is_some_and(|w| pending(&w.diff)) {
        fx.push(Effect::HighlightDiff { workdir, target: target.clone(), generation: st.diff_shown_gen, diff: std::sync::Arc::clone(d) });
    }
}

/// Attaches a background result to the diff it was for, if that is still on screen and still
/// pending. Stale results (another file, the diff closed, a newer diff shown) are dropped. Only
/// the spans change: never the scroll position, cursor, selection or folds.
pub(super) fn handle(st: &mut AppState, msg: Msg) {
    let Msg::DiffHighlighted { target, generation, hunks, whole } = msg else { return };
    if generation != st.diff_shown_gen {
        return;
    }
    let Some((_, d)) = st.diff.as_mut().filter(|(t, _)| *t == target) else { return };
    // Copies the diff only if a reload in flight still shares it.
    let d = std::sync::Arc::make_mut(d);
    if let Some(h) = hunks {
        attach(d, h);
    }
    // The whole file is gone if the view switched back to hunks meanwhile.
    if let (Some(h), Some(w)) = (whole, d.whole.as_deref_mut()) {
        attach(&mut w.diff, h);
    }
}

fn attach(d: &mut FileDiff, h: DiffHighlight) {
    let shape: Vec<usize> = d.hunks.iter().map(|h| h.lines.len()).collect();
    if pending(d) && h.hunk_lens() == shape {
        d.highlight = Some(h);
    }
}
