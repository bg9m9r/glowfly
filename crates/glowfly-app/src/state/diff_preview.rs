//! The binary diff on screen's two sides: their sizes and decoded images, read in the background
//! (`Effect::LoadBinarySides`, set only by update/diff_preview.rs) for the header's size chip and
//! the before/after image preview.

use super::AppState;
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

/// The loaded sides when they are for the diff on screen.
pub fn shown(st: &AppState) -> Option<&Arc<BinarySides>> {
    let (target, _) = st.diff.as_ref()?;
    st.session.diff_preview.as_ref().filter(|p| p.target == *target)?.loaded()
}
