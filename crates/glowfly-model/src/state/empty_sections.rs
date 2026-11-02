//! Left-panel sections that start collapsed while they have nothing to list (Remote, Tags,
//! Stashes, Worktrees, Submodules): while empty, a section's header toggles its own expanded
//! flag here, not its usual open flag, so it is collapsed each time a repository opens with it
//! empty and the user's choice for the populated section stays as it was. A section that empties
//! in front of the user (the last stash popped) keeps its open state rather than folding away.
//! Per tab (`TabView::empty_sections`), never saved; the sidebar asks for the flag each frame.

use std::path::{Path, PathBuf};

/// A section that folds away while empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folding {
    Remote,
    Tags,
    Stashes,
    Worktrees,
    Submodules,
}

const COUNT: usize = 5;

/// Each folding section's expanded flag while empty, for the repository they were set in.
#[derive(Default)]
pub struct EmptySections {
    repo: Option<PathBuf>,
    open: [bool; COUNT],
    /// It had rows the last time it was drawn.
    had_rows: [bool; COUNT],
}

impl EmptySections {
    /// The flag `which`'s header toggles in `repo` with `count` rows: the section's own `open`
    /// while it has rows; while empty, its expanded flag here, collapsed until the user opens it
    /// (or, when it has just emptied, as `open` was). Another repository starts afresh.
    pub fn open_mut<'a>(&'a mut self, repo: &Path, which: Folding, count: usize, open: &'a mut bool) -> &'a mut bool {
        if self.repo.as_deref() != Some(repo) {
            *self = Self { repo: Some(repo.to_owned()), ..Self::default() };
        }
        let i = which as usize;
        if count > 0 {
            self.had_rows[i] = true;
            return open;
        }
        if std::mem::take(&mut self.had_rows[i]) {
            self.open[i] = *open;
        }
        &mut self.open[i]
    }
}

#[cfg(test)]
#[path = "empty_sections_tests.rs"]
mod tests;
