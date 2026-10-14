//! The New tag dialog's form, and the name and remote checks its view and `update` share.

use super::branches::default_remote;
use trellis_core::repo::Snapshot;
use trellis_core::tag_ops::{TagNameProblem, check_tag_name};

/// What the New tag dialog is editing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagForm {
    /// The commit to tag (a full sha).
    pub at: String,
    pub name: String,
    pub message: String,
    /// The message is required.
    pub annotated: bool,
    /// The remote picked for the push ("" when the repository has none).
    pub remote: String,
}

/// What is wrong with `name` (trimmed here) as a new tag, given the snapshot's tags; None when
/// it can be created.
pub fn tag_problem(snap: Option<&Snapshot>, name: &str) -> Option<TagNameProblem> {
    let existing: Vec<&str> =
        snap.map(|s| s.tags.iter().map(|(n, _)| n.as_str()).chain(s.other_tags.iter().map(String::as_str)).collect()).unwrap_or_default();
    check_tag_name(name.trim(), &existing).err()
}

/// The remote a new tag is pushed to: `form_remote` while it still exists, else the default
/// remote; None when there are no remotes.
pub fn push_remote(snap: &Snapshot, form_remote: &str) -> Option<String> {
    if snap.remotes.iter().any(|(r, _)| r == form_remote) { Some(form_remote.to_owned()) } else { default_remote(snap) }
}
