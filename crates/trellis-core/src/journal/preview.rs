//! A journal state's preview: its first-parent history, and how many commits restoring it would
//! take off its subject (leaving) and bring back (arriving), counted up to `MAX_COUNT`.

use crate::error::{Error, Result};
use git2::{Oid, Repository};
use std::path::Path;

/// The most commits shown in the preview's history.
pub const HISTORY: usize = 8;
/// The most commits a count walks before it stops with `more`.
pub const MAX_COUNT: usize = 1000;

/// One commit of the preview's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitLine {
    pub oid: Oid,
    pub summary: String,
    pub author: String,
    /// Unix seconds.
    pub time: i64,
}

/// A count of commits; `more` when the walk stopped at `MAX_COUNT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count {
    pub n: usize,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    /// First-parent, newest first, at most `HISTORY`.
    pub history: Vec<CommitLine>,
    /// The commits the subject has now that restoring `oid` takes off it.
    pub leaving: Option<Count>,
    /// The commits `oid` has that the subject lacks now.
    pub arriving: Option<Count>,
}

/// The commits reachable from `from` but not from `hide`.
fn count(repo: &Repository, from: Oid, hide: Oid) -> Result<Count> {
    let mut walk = repo.revwalk()?;
    walk.push(from)?;
    walk.hide(hide)?;
    let mut n = 0;
    for id in walk {
        id?;
        if n == MAX_COUNT {
            return Ok(Count { n, more: true });
        }
        n += 1;
    }
    Ok(Count { n, more: false })
}

/// The preview of commit `oid` against the subject's tip now (`against`; `None` counts
/// nothing). A missing commit is `Error::NotFound`.
pub fn preview(workdir: &Path, oid: Oid, against: Option<Oid>) -> Result<Preview> {
    let repo = Repository::open(workdir)?;
    let Ok(commit) = repo.find_commit(oid) else { return Err(Error::NotFound(format!("commit {oid}"))) };
    let mut history = Vec::new();
    let mut next = Some(commit);
    while let Some(c) = next.take() {
        history.push(CommitLine {
            oid: c.id(),
            summary: c.summary().unwrap_or_default().to_owned(),
            author: c.author().name().unwrap_or_default().to_owned(),
            time: c.time().seconds(),
        });
        if history.len() == HISTORY {
            break;
        }
        next = c.parent(0).ok();
    }
    let against = against.filter(|a| *a != oid && repo.find_commit(*a).is_ok());
    let (leaving, arriving) = match against {
        Some(a) => (Some(count(&repo, a, oid)?), Some(count(&repo, oid, a)?)),
        None => (None, None),
    };
    Ok(Preview { history, leaving, arriving })
}
