//! The open conflict: the file's chunks and the side(s) picked for each.

use trellis_core::repo::{Choice, ConflictContent, ConflictFile, render_output};

#[derive(Clone, Debug)]
pub struct ConflictView {
    /// Repo-relative path, `/`-separated.
    pub path: String,
    /// `None` while loading.
    pub file: Option<ConflictFile>,
    /// One per chunk.
    pub choices: Vec<Choice>,
    /// The chunk the navigation is on.
    pub current: usize,
    /// A save (or whole-side resolve) is running.
    pub saving: bool,
}

impl ConflictView {
    pub fn loading(path: String) -> Self {
        Self { path, file: None, choices: Vec::new(), current: 0, saving: false }
    }

    pub fn set_file(&mut self, f: ConflictFile) {
        self.choices = vec![Choice::Unresolved; f.chunk_count()];
        self.current = 0;
        self.file = Some(f);
    }

    pub fn chunk_count(&self) -> usize {
        self.choices.len()
    }

    /// Chunks with no pick yet.
    pub fn unresolved(&self) -> usize {
        self.choices.iter().filter(|c| **c == Choice::Unresolved).count()
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
