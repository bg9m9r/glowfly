//! The open conflict: the file's chunks and the side(s) picked for each, some of them picked
//! automatically when that is safe (`trellis_core::conflict_rows::trivial_pick`).

use trellis_core::conflict_rows::{Trivial, trivial_picks};
use trellis_core::repo::{Choice, ConflictContent, ConflictFile, render_output};

#[derive(Clone, Debug)]
pub struct ConflictView {
    /// Repo-relative path, `/`-separated.
    pub path: String,
    /// `None` while loading.
    pub file: Option<ConflictFile>,
    /// One per chunk.
    pub choices: Vec<Choice>,
    /// One per chunk: why its pick was made automatically, until the user changes it.
    pub auto: Vec<Option<Trivial>>,
    /// The chunk the navigation is on.
    pub current: usize,
    /// A save (or whole-side resolve) is running.
    pub saving: bool,
    /// Trellis itself is resolving this path (Save, a whole side, Mark resolved): once it is no
    /// longer conflicted the view opens the next conflicted file instead of closing.
    pub step_on_resolve: bool,
}

impl ConflictView {
    pub fn loading(path: String) -> Self {
        Self { path, file: None, choices: Vec::new(), auto: Vec::new(), current: 0, saving: false, step_on_resolve: false }
    }

    /// Shows `f`, with the chunks that are safe to decide picked already (nothing is written
    /// until Save), on the first chunk still to pick.
    pub fn set_file(&mut self, f: ConflictFile) {
        let picks = trivial_picks(&f);
        self.choices = picks.iter().map(|p| p.map_or(Choice::Unresolved, |p| p.0)).collect();
        self.auto = picks.iter().map(|p| p.map(|p| p.1)).collect();
        self.file = Some(f);
        self.current = self.first_open();
    }

    pub fn chunk_count(&self) -> usize {
        self.choices.len()
    }

    /// Chunks with no pick yet.
    pub fn unresolved(&self) -> usize {
        self.choices.iter().filter(|c| **c == Choice::Unresolved).count()
    }

    /// Chunks whose pick was made automatically and not changed since.
    pub fn auto_count(&self) -> usize {
        self.auto.iter().filter(|a| a.is_some()).count()
    }

    /// The first chunk with no pick, or 0 when every chunk has one.
    pub fn first_open(&self) -> usize {
        self.choices.iter().position(|c| *c == Choice::Unresolved).unwrap_or(0)
    }

    /// The file as it would be saved, once every chunk has a pick.
    pub fn output(&self) -> Option<String> {
        render_output(self.file.as_ref()?, &self.choices)
    }

    /// Every chunk has a pick (the same condition as `output` being `Some`, without rendering).
    pub fn can_save(&self) -> bool {
        let text =
            self.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)) && f.chunk_count() == self.choices.len());
        !self.saving && text && self.unresolved() == 0
    }
}
