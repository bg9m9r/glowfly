//! The binary diff on screen's two sides: their sizes and decoded images, read in the background
//! (`DiffEffect::BinarySides`, set only by update/diff_preview.rs) for the header's size chip and
//! the before/after image preview. A changed SVG that git takes for text is drawn there too
//! when the header's Text / Preview switch says Preview (`svg_switch`, `drawn`).

use super::AppState;
use glowfly_core::preview_svg::is_svg_path;
use glowfly_core::repo::{BinarySides, DiffTarget};
use std::sync::Arc;

/// The sides of the binary diff `target`, requested for the diff shown at `generation`.
#[derive(Clone, Debug)]
pub struct DiffPreview {
    pub target: DiffTarget,
    pub generation: u64,
    /// `None` while they load; an error leaves the body on its plain binary state.
    pub sides: Option<Result<Arc<BinarySides>, String>>,
}

impl DiffPreview {
    /// The loaded sides, when they loaded.
    pub fn loaded(&self) -> Option<&Arc<BinarySides>> {
        self.sides.as_ref().and_then(|s| s.as_ref().ok())
    }
}

/// The diff on screen's Text / Preview switch: `Some(on)` for an SVG that git diffs as text
/// (and no Git LFS summary or pointer text in its place), `on` when Preview is chosen.
pub fn svg_switch(st: &AppState) -> Option<bool> {
    let (target, d) = st.session.diff.view.as_ref()?;
    let lfs = super::lfs_notes::summary(st).is_some() || super::lfs_notes::pointer_shown(st);
    (!d.binary && !lfs && is_svg_path(target.path())).then_some(st.settings.diff_svg_preview)
}

/// Whether the diff on screen shows its sides drawn instead of text rows: a binary file, or an
/// SVG with Preview chosen.
pub fn drawn(st: &AppState) -> bool {
    st.session.diff.view.as_ref().is_some_and(|(_, d)| d.binary) || svg_switch(st) == Some(true)
}

/// The loaded sides when they are for the diff on screen.
pub fn shown(st: &AppState) -> Option<&Arc<BinarySides>> {
    let (target, _) = st.session.diff.view.as_ref()?;
    st.session.diff_preview.as_ref().filter(|p| p.target == *target)?.loaded()
}
