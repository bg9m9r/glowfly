//! The open conflict: the file's chunks and what each resolves to (whole sides, single lines or
//! text typed in the output), some picked automatically when that is safe
//! (`trellis_core::conflict_rows::trivial_pick`), and the output's text while it is edited.

use trellis_core::conflict_edit::{EditText, edit_text};
use trellis_core::conflict_pick::{ChunkPick, render_picks};
use trellis_core::conflict_rows::{Trivial, trivial_picks};
use trellis_core::repo::{Chunk, ConflictContent, ConflictFile};

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
    /// The chunk the navigation is on.
    pub current: usize,
    /// A save (or whole-side resolve) is running.
    pub saving: bool,
    /// Trellis itself is resolving this path (Save, a whole side, Mark resolved): once it is no
    /// longer conflicted the view opens the next conflicted file instead of closing.
    pub step_on_resolve: bool,
    /// The output as text, while it is being edited.
    pub edit: Option<OutputEdit>,
    /// Why the last edit of the output was undone (it touched a clean line or two conflicts).
    pub edit_note: Option<String>,
    /// The toast for a Save with markers that is running.
    pub saved_note: Option<String>,
}

/// The output being edited: `buffer` is the editor's text, `rendered` the text as last built
/// from the picks, with each chunk's range.
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
            current: 0,
            saving: false,
            step_on_resolve: false,
            edit: None,
            edit_note: None,
            saved_note: None,
        }
    }

    /// Shows `f`, with the chunks that are safe to decide picked already (nothing is written
    /// until Save), on the first chunk still to pick.
    pub fn set_file(&mut self, f: ConflictFile) {
        let picks = trivial_picks(&f);
        self.picks = picks.iter().map(|p| p.map(|p| ChunkPick::Sides(p.0)).unwrap_or_default()).collect();
        self.auto = picks.iter().map(|p| p.map(|p| p.1)).collect();
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

    /// Chunks not resolved yet: no pick, or typed text with markers still in it.
    pub fn unresolved(&self) -> usize {
        self.picks.iter().filter(|p| p.is_open()).count()
    }

    /// Chunks whose pick was made automatically and not changed since.
    pub fn auto_count(&self) -> usize {
        self.auto.iter().filter(|a| a.is_some()).count()
    }

    /// Chunks the user decided (a side, lines or typed text) that aren't saved: those not picked
    /// automatically and not left unresolved. What closing the view would lose.
    pub fn user_choices(&self) -> usize {
        let default = ChunkPick::default();
        self.picks.iter().enumerate().filter(|&(k, p)| self.auto.get(k).is_none_or(Option::is_none) && *p != default).count()
    }

    /// The first chunk not resolved yet, or 0 when every chunk is.
    pub fn first_open(&self) -> usize {
        self.picks.iter().position(ChunkPick::is_open).unwrap_or(0)
    }

    /// The file as it would be saved, once every chunk is resolved.
    pub fn output(&self) -> Option<String> {
        render_picks(self.file.as_ref()?, &self.picks, false, false, None)
    }

    /// The file as Save with markers writes it: chunks not resolved yet as conflict markers.
    pub fn partial_output(&self) -> Option<String> {
        render_picks(self.file.as_ref()?, &self.picks, true, false, None)
    }

    fn is_text(&self) -> bool {
        self.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)) && f.chunk_count() == self.picks.len())
    }

    /// Every chunk is resolved (the same condition as `output` being `Some`, without rendering).
    pub fn can_save(&self) -> bool {
        !self.saving && self.is_text() && self.unresolved() == 0
    }

    /// Some chunks are resolved (picked or edited) and some aren't yet: Save with markers.
    pub fn can_save_partial(&self) -> bool {
        !self.saving && self.is_text() && self.unresolved() > 0 && self.picks.iter().any(|p| !p.is_open() || p.is_edited())
    }

    /// While the output is edited: its text built again from the picks.
    pub fn rebuild_edit(&mut self) {
        if self.edit.is_none() {
            return;
        }
        let rendered = self.file.as_ref().and_then(|f| edit_text(f, &self.picks));
        self.edit = rendered.map(|rendered| OutputEdit { buffer: rendered.text.clone(), rendered });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::repo::Choice;

    #[test]
    fn user_choices_skip_automatic_and_open_chunks() {
        let mut v = ConflictView::loading("a.txt".into());
        v.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::default(), ChunkPick::Sides(Choice::Theirs)];
        v.auto = vec![None, None, Some(Trivial::Identical)];
        assert_eq!(v.user_choices(), 1, "the automatic pick and the open chunk don't count");
        v.picks[1] = ChunkPick::Edited { lines: vec!["x\n".into()], before: Box::new(ChunkPick::default()) };
        assert_eq!(v.user_choices(), 2, "typed output counts");
        v.auto[2] = None;
        assert_eq!(v.user_choices(), 3, "an automatic pick the user changed counts");
        assert_eq!(ConflictView::loading("b.txt".into()).user_choices(), 0);
    }
}
