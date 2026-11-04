//! Reading the open diff (`Effect::Diff`, runner/diff.rs): the diff itself, finishing its syntax
//! highlighting, the unchanged lines between its hunks and a binary diff's sides. Reads only,
//! scoped to the repository's tab.

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::repo::{DiffTarget, FileDiff, FileEntry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub enum DiffEffect {
    /// Loads `target`'s diff for the viewer.
    Load {
        workdir: PathBuf,
        target: DiffTarget,
        reload: bool,
        generation: u64,
        /// On a reload, the diff on screen: unchanged hunks keep its highlighting.
        previous: Option<Arc<FileDiff>>,
        /// Hide whitespace-only changes (the diff_ignore_whitespace setting when the request was made).
        ignore_whitespace: bool,
        /// Also load the whole file (the diff_mode setting when the request was made).
        whole_file: bool,
        /// `target`'s row in the list loaded with its whole commit or comparison
        /// (`diff_nav::file_order::listed`), so only the file and its rename partner are read.
        listed: Option<FileEntry>,
    },
    /// Finishes the syntax highlighting `diff` arrived with (pending in its hunks or its whole
    /// file) for the diff shown at `generation` (DiffSlot::shown_gen). Started by
    /// Runner::run with a cancel ticket: a newer `Highlight` cancels this one.
    Highlight { workdir: PathBuf, target: DiffTarget, generation: u64, diff: Arc<FileDiff> },
    /// Reads the unchanged lines between `diff`'s hunks for the hunk view, for the diff shown at
    /// `generation` (DiffSlot::shown_gen).
    Gaps { workdir: PathBuf, target: DiffTarget, generation: u64, diff: Arc<FileDiff> },
    /// Reads the sizes and decodes the images of the binary diff shown at `generation`
    /// (DiffSlot::shown_gen), for its header and preview.
    BinarySides { workdir: PathBuf, target: DiffTarget, generation: u64 },
}

impl EffectMeta for DiffEffect {
    fn name(&self) -> &'static str {
        match self {
            DiffEffect::Load { .. } => "load-diff",
            DiffEffect::Highlight { .. } => "highlight",
            DiffEffect::Gaps { .. } => "load-diff-gaps",
            DiffEffect::BinarySides { .. } => "binary-sides",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            DiffEffect::Load { workdir, .. }
            | DiffEffect::Highlight { workdir, .. }
            | DiffEffect::Gaps { workdir, .. }
            | DiffEffect::BinarySides { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
