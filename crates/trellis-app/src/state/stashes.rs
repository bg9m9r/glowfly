//! The Rename stash dialog's form (`Modal::RenameStash`) and the Stash dialog's form
//! (`Modal::StashPush`): what goes into a new stash, the files it lists and why it can't run.

use git2::Oid;
use std::collections::{BTreeMap, BTreeSet};
use trellis_core::repo::{FileEntry, Snapshot, StashInfo};
use trellis_core::stash_ops::{StashScope, edited_message, split_message};

/// Why a stash can't run: nothing has changed.
pub const NOTHING: &str = "There are no changes to stash.";
/// Why a stash can't run: git can't stash with unmerged index entries.
pub const CONFLICTED: &str = "Resolve the conflicted files before stashing.";
/// Why a staged-only stash can't run.
pub const NOTHING_STAGED: &str = "Nothing is staged. Stage the changes to set aside first, or choose another option.";
/// Why a stash of selected files can't run.
pub const NO_FILES: &str = "Choose at least one file to stash.";

/// Renaming one stash list entry: the field holds the part after "On <branch>: ".
pub struct RenameStashForm {
    pub oid: Oid,
    /// Its position when the dialog opened (`stash@{index}`).
    pub index: usize,
    /// The full stash list text when the dialog opened.
    pub original: String,
    /// The editable text.
    pub message: String,
}

impl RenameStashForm {
    pub fn new(s: &StashInfo) -> Self {
        let message = split_message(&s.message).1.to_owned();
        Self { oid: s.oid, index: s.index, original: s.message.clone(), message }
    }

    /// The full stash list text the field gives; None while it is blank.
    pub fn new_message(&self) -> Option<String> {
        edited_message(&self.original, &self.message)
    }

    /// Whether the field gives a message different from the current one. The untouched text
    /// counts as unchanged even for a "WIP on" entry, which saving would turn into "On".
    pub fn changed(&self) -> bool {
        let untouched = edited_message(&self.original, split_message(&self.original).1);
        self.new_message().is_some_and(|m| m != self.original && Some(&m) != untouched.as_ref())
    }

    /// The branch the stash was made on, from its message.
    pub fn branch(&self) -> Option<&str> {
        split_message(&self.original).0
    }
}

/// The Stash dialog: what to stash, and the message.
pub struct StashPushForm {
    pub scope: StashScope,
    pub message: String,
    /// Paths ticked under Selected files, kept across refreshes.
    pub chosen: BTreeSet<String>,
    /// Focus the message field on the next frame (true when opened).
    pub focus_message: bool,
}

impl StashPushForm {
    /// The dialog on `scope`, with only `path` ticked (or nothing).
    pub fn new(scope: StashScope, path: Option<String>) -> Self {
        Self { scope, message: String::new(), chosen: path.into_iter().collect(), focus_message: true }
    }

    /// The ticked paths that are still changed in `snap`, in list order.
    pub fn paths(&self, snap: &Snapshot) -> Vec<String> {
        wip_files(snap).into_iter().map(|f| f.path).filter(|p| self.chosen.contains(p)).collect()
    }
}

/// The working-tree files: unstaged and staged entries merged by path (the unstaged status
/// wins), sorted by path, untracked included.
pub fn wip_files(snap: &Snapshot) -> Vec<FileEntry> {
    let mut by_path: BTreeMap<&str, char> = snap.staged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    by_path.extend(snap.unstaged.iter().map(|f| (f.path.as_str(), f.status)));
    by_path.into_iter().map(|(path, status)| FileEntry { path: path.to_owned(), status }).collect()
}

/// Why stashing `scope` (with `paths` for Selected files) can't run now, or None.
pub fn push_blocked(snap: &Snapshot, scope: StashScope, paths: &[String]) -> Option<&'static str> {
    if !snap.is_dirty() {
        Some(NOTHING)
    } else if snap.unstaged.iter().any(|f| f.status == '!') {
        Some(CONFLICTED)
    } else if scope == StashScope::Staged && snap.staged.is_empty() {
        Some(NOTHING_STAGED)
    } else if scope == StashScope::Files && paths.is_empty() {
        Some(NO_FILES)
    } else {
        None
    }
}

/// The job label for stashing `scope`.
pub fn label(scope: StashScope) -> &'static str {
    match scope {
        StashScope::All => "Stash",
        StashScope::Staged => "Stash staged changes",
        StashScope::Files => "Stash selected files",
    }
}
