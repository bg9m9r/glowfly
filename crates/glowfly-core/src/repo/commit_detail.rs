//! A commit's details for the details panel: its message and people, decoded (commit_text) and
//! shown as the mailmap has them, and its files against the first parent; and a commit's whole
//! message alone, for Copy > Message.

use super::CommitDetail;
use super::diff::{absent_paths, commit_diff, file_entries};
use crate::commit_text;
use crate::error::Result;
use git2::Oid;
use std::path::Path;

/// The details of commit `oid`: message, author, committer, times, parents and changed files.
pub fn commit_detail(workdir: &Path, oid: Oid) -> Result<CommitDetail> {
    let repo = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir)?;
    let c = repo.find_commit(oid)?;
    let diff = commit_diff(&repo, oid, None)?;
    let mut files = file_entries(&diff, false);
    super::lfs_marks::mark_delta_files(&repo, &diff, &mut files, false);
    let absent = absent_paths(&repo, &files);
    let map = crate::mailmap::MailmapText::read(&repo).open();
    let (author, committer) = (map.person(commit_text::author(&c)), map.person(commit_text::committer(&c)));
    Ok(CommitDetail {
        oid,
        message: shown_message(&c),
        author: author.name,
        email: author.email,
        committer: committer.name,
        committer_email: committer.email,
        time: c.time().seconds(),
        author_time: c.author().when().seconds(),
        parents: c.parent_ids().collect(),
        files,
        absent,
        signature: crate::signature_check::signature_format(&repo, oid),
    })
}

/// The message as the details panel shows it: decoded, without trailing whitespace.
fn shown_message(c: &git2::Commit<'_>) -> String {
    commit_text::message(c).trim_end().to_owned()
}

/// Commit `oid`'s whole message as the details panel shows it (decoded, no trailing line break),
/// for Copy > Message.
pub fn commit_message(workdir: &Path, oid: Oid) -> Result<String> {
    let repo = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir)?;
    let c = repo.find_commit(oid)?;
    Ok(shown_message(&c))
}

#[cfg(test)]
#[path = "commit_detail_tests.rs"]
mod tests;
