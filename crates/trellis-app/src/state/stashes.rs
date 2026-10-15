//! The Rename stash dialog's form (`Modal::RenameStash`).

use git2::Oid;
use trellis_core::repo::StashInfo;
use trellis_core::stash_ops::{edited_message, split_message};

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
