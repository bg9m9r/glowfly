//! A commit's details for the details panel: its message and people, decoded (commit_text), and
//! its files against the first parent.

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
    let (author, committer) = (commit_text::author(&c), commit_text::committer(&c));
    Ok(CommitDetail {
        oid,
        message: commit_text::message(&c).trim_end().to_owned(),
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
