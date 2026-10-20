//! Reading the reflogs the journal shows, with libgit2: the worktree's HEAD (per worktree) and
//! every local branch (shared), newest `MAX_PER_REF` entries each. Entries whose new id is all
//! zeros and branches whose names aren't UTF-8 are skipped; messages are read lossily.

use super::{MAX_PER_REF, Subject};
use crate::error::Result;
use crate::undo::HeadPos;
use git2::{BranchType, Oid, Repository};

/// One reflog entry, as read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RawEntry {
    pub subject: Subject,
    /// The id before (`None`: the ref was created).
    pub old: Option<Oid>,
    pub new: Oid,
    /// Unix seconds, as the entry's committer recorded them.
    pub at: i64,
    pub message: String,
    /// Its place in its reflog, 0 for the newest.
    pub index: usize,
}

/// What `read` found.
pub(super) struct Read {
    pub entries: Vec<RawEntry>,
    /// Some reflog had more than `MAX_PER_REF` entries.
    pub capped: bool,
    pub head: (Option<HeadPos>, Option<Oid>),
    /// Each local branch and its tip.
    pub tips: Vec<(String, Oid)>,
}

/// The newest `MAX_PER_REF` entries of the reflog `name` (none when it has no reflog).
fn entries_of(repo: &Repository, name: &str, subject: &Subject, out: &mut Vec<RawEntry>) -> bool {
    let Ok(log) = repo.reflog(name) else { return false };
    for (index, e) in log.iter().take(MAX_PER_REF).enumerate() {
        let new = e.id_new();
        if new.is_zero() {
            continue;
        }
        let old = Some(e.id_old()).filter(|o| !o.is_zero());
        let message = String::from_utf8_lossy(e.message_bytes().unwrap_or_default()).into_owned();
        out.push(RawEntry { subject: subject.clone(), old, new, at: e.committer().when().seconds(), message, index });
    }
    log.len() > MAX_PER_REF
}

pub(super) fn read(repo: &Repository) -> Result<Read> {
    let mut entries = Vec::new();
    let mut capped = entries_of(repo, "HEAD", &Subject::Head, &mut entries);
    let mut tips = Vec::new();
    for b in repo.branches(Some(BranchType::Local))? {
        let (b, _) = b?;
        let Ok(Some(name)) = b.name() else { continue };
        let name = name.to_owned();
        if let Some(tip) = b.get().target() {
            tips.push((name.clone(), tip));
        }
        capped |= entries_of(repo, &format!("refs/heads/{name}"), &Subject::Branch(name), &mut entries);
    }
    Ok(Read { entries, capped, head: crate::undo::head(repo), tips })
}
