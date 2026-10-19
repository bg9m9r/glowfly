//! The open conflict: the file's chunks and what each resolves to (whole sides, single lines or
//! text typed in the output), some picked automatically when that is safe
//! (`trellis_core::conflict_rows::trivial_pick`) and some brought back from a file saved with
//! markers (`trellis_core::conflict_restore`), the clean runs edited in the output, the folds
//! opened, and the output's text while it is edited.

use crate::msg::{ConflictMsg, Msg};
use std::collections::BTreeSet;
use trellis_core::conflict_edit::{EditText, edit_text};
use trellis_core::conflict_pick::{ChunkPick, CleanEdits, render};
use trellis_core::conflict_restore::SavedPicks;
use trellis_core::conflict_rows::{Trivial, trivial_picks};
use trellis_core::i18n::Lang;
use trellis_core::repo::{Chunk, ConflictContent, ConflictFile, Segment};

#[derive(Clone, Debug)]
pub struct ConflictView {
    /// Repo-relative path, `/`-separated.
    pub path: String,
    /// `None` while loading.
    pub file: Option<ConflictFile>,
    /// One per chunk.
    pub picks: Vec<ChunkPick>,
    /// One per chunk: why its pick was made automatically, until the user changes it.
    pub auto: Vec<Option<Trivial>>,
    /// Clean runs edited in the output, by segment.
    pub cleans: CleanEdits,
    /// Clean runs whose fold the user opened, by segment (panes and output alike).
    pub opened: BTreeSet<usize>,
    /// What the working file holds, as far as the view knows: the picks and clean edits brought
    /// back when it loaded (all nothing when none were) or written by the last Save with
    /// markers. Choices equal to it are not lost by closing.
    pub baseline: (Vec<ChunkPick>, CleanEdits),
    /// How many chunks came back from the working file with a pick when it loaded.
    pub restored: usize,
    /// The chunk the navigation is on.
    pub current: usize,
    /// A save (or whole-side resolve) is running.
    pub saving: bool,
    /// Trellis itself is resolving this path (Save, a whole side, Mark resolved): once it is no
    /// longer conflicted the view opens the next conflicted file instead of closing.
    pub step_on_resolve: bool,
    /// The output as text, while it is being edited.
    pub edit: Option<OutputEdit>,
    /// Why the last edit of the output was undone (it spanned a conflict and other lines).
    pub edit_note: Option<String>,
    /// A Save with markers that is running: its toast, and the picks and clean edits it writes
    /// (the baseline once it worked).
    pub pending_save: Option<(String, Vec<ChunkPick>, CleanEdits)>,
}

/// The output being edited: `buffer` is the editor's text, `rendered` the text as last built
/// from the picks and clean edits, with each chunk's and clean run's range.
#[derive(Clone, Debug)]
pub struct OutputEdit {
    pub buffer: String,
    pub rendered: EditText,
}

impl ConflictView {
    pub fn loading(path: String) -> Self {
        Self {
            path,
            file: None,
            picks: Vec::new(),
            auto: Vec::new(),
            cleans: CleanEdits::new(),
            opened: BTreeSet::new(),
            baseline: (Vec::new(), CleanEdits::new()),
            restored: 0,
            current: 0,
            saving: false,
            step_on_resolve: false,
            edit: None,
            edit_note: None,
            pending_save: None,
        }
    }

    /// Shows `f` as `set_file_saved` does, with nothing saved in the working file.
    #[cfg(test)]
    pub fn set_file(&mut self, f: ConflictFile) {
        self.set_file_saved(f, None);
    }

    /// Shows `f`, with the chunks that are safe to decide picked already (nothing is written
    /// until Save) and what `saved` holds brought back over them, on the first chunk still to
    /// pick. A pick brought back that is the chunk's automatic one keeps its tag. Folds close.
    pub fn set_file_saved(&mut self, f: ConflictFile, saved: Option<SavedPicks>) {
        let trivial = trivial_picks(&f);
        self.picks = trivial.iter().map(|p| p.map(|p| ChunkPick::Sides(p.0)).unwrap_or_default()).collect();
        self.auto = trivial.iter().map(|p| p.map(|p| p.1)).collect();
        self.baseline = (vec![ChunkPick::default(); self.picks.len()], CleanEdits::new());
        self.restored = 0;
        if let Some(s) = saved.filter(|s| s.picks.len() == self.picks.len()) {
            for (k, p) in s.picks.iter().enumerate().filter(|(_, p)| **p != ChunkPick::default()) {
                self.auto[k] = trivial[k].filter(|t| *p == ChunkPick::Sides(t.0)).map(|t| t.1);
                self.picks[k] = p.clone();
                self.restored += 1;
            }
            self.baseline = (s.picks, s.cleans);
        }
        self.cleans = self.baseline.1.clone();
        self.opened.clear();
        self.file = Some(f);
        self.current = self.first_open();
        self.rebuild_edit();
    }

    pub fn chunk_count(&self) -> usize {
        self.picks.len()
    }

    /// Chunk `k` of the file.
    pub fn chunk(&self, k: usize) -> Option<&Chunk> {
        self.file.as_ref()?.chunks().nth(k)
    }

    /// The file's segments, for a text file.
    pub fn segments(&self) -> Option<&[Segment]> {
        match &self.file.as_ref()?.content {
            ConflictContent::Text(segs) => Some(segs),
            _ => None,
        }
    }

    /// Chunks not resolved yet: no pick, or typed text with markers still in it.
    pub fn unresolved(&self) -> usize {
        self.picks.iter().filter(|p| p.is_open()).count()
    }

    /// Chunks resolved so far, by the user or automatically.
    pub fn picked(&self) -> usize {
        self.chunk_count() - self.unresolved()
    }

    /// What Reset picks sends: the reset itself, behind a confirmation once anything is picked.
    pub fn reset_msg(&self, t: Lang) -> Msg {
        let reset = Msg::Conflict(ConflictMsg::ConflictReset);
        match self.picked() {
            0 => reset,
            n => Msg::confirm_danger(
                t.tr("conflict.header.reset.confirm.title"),
                t.trn("conflict.header.reset.confirm.body", n, &[]),
                t.tr("conflict.header.reset.confirm.button"),
                reset,
            ),
        }
    }

    /// Chunks whose pick was made automatically and not changed since.
    pub fn auto_count(&self) -> usize {
        self.auto.iter().filter(|a| a.is_some()).count()
    }

    /// The user's decisions that aren't saved: chunks decided (a side, lines or typed text),
    /// not picked automatically and not as the working file holds them, plus clean runs edited
    /// differently from it. What closing the view would lose.
    pub fn user_choices(&self) -> usize {
        let default = ChunkPick::default();
        let picks = self
            .picks
            .iter()
            .enumerate()
            .filter(|&(k, p)| self.auto.get(k).is_none_or(Option::is_none) && *p != default && self.baseline.0.get(k) != Some(p))
            .count();
        let segs: BTreeSet<&usize> = self.cleans.keys().chain(self.baseline.1.keys()).collect();
        picks + segs.into_iter().filter(|s| self.cleans.get(s) != self.baseline.1.get(s)).count()
    }

    /// The first chunk not resolved yet, or 0 when every chunk is.
    pub fn first_open(&self) -> usize {
        self.picks.iter().position(ChunkPick::is_open).unwrap_or(0)
    }

    /// The file as it would be saved, once every chunk is resolved.
    pub fn output(&self) -> Option<String> {
        render(self.file.as_ref()?, &self.picks, &self.cleans, false, false, None)
    }

    /// The file as Save with markers writes it: chunks not resolved yet as conflict markers.
    pub fn partial_output(&self) -> Option<String> {
        render(self.file.as_ref()?, &self.picks, &self.cleans, true, false, None)
    }

    fn is_text(&self) -> bool {
        self.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)) && f.chunk_count() == self.picks.len())
    }

    /// Every chunk is resolved (the same condition as `output` being `Some`, without rendering).
    pub fn can_save(&self) -> bool {
        !self.saving && self.is_text() && self.unresolved() == 0
    }

    /// Some chunks aren't resolved yet, and something is decided: a chunk picked or edited, or
    /// a clean run edited. Save with markers.
    pub fn can_save_partial(&self) -> bool {
        let decided = self.picks.iter().any(|p| !p.is_open() || p.is_edited()) || !self.cleans.is_empty();
        !self.saving && self.is_text() && self.unresolved() > 0 && decided
    }

    /// While the output is edited: its text built again from the picks and clean edits.
    pub fn rebuild_edit(&mut self) {
        if self.edit.is_none() {
            return;
        }
        let rendered = self.file.as_ref().and_then(|f| edit_text(f, &self.picks, &self.cleans));
        self.edit = rendered.map(|rendered| OutputEdit { buffer: rendered.text.clone(), rendered });
    }
}

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
