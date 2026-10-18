//! Reading the unchanged lines between a diff's hunks, for the hunk view (`Effect::LoadDiffGaps`).

use crate::msg::{DiffMsg, Msg};
use std::path::PathBuf;
use std::sync::Arc;
use trellis_core::repo::{self, DiffTarget, FileDiff};

/// The text of every unchanged run between `diff`'s hunks, colored in its highlight's syntax.
pub(super) fn load_job(workdir: PathBuf, target: DiffTarget, generation: u64, diff: Arc<FileDiff>) -> Msg {
    let syntax = diff.highlight.as_ref().map(|h| h.syntax.as_str());
    let result = repo::load_gap_text(&workdir, &target, &diff.hunks, syntax).map(Arc::new).map_err(|e| e.to_string());
    Msg::Diff(DiffMsg::GapsLoaded { target, generation, result })
}
