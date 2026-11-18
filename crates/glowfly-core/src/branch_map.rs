//! Each local branch's drift from the main branch and the pairs of branches that would conflict.
//! This file holds the types, the plan made from a loaded snapshot (`request`, pure) and the
//! questions the view asks of a finished map; `read.rs` measures the branches and `forecast.rs`
//! makes the trial merges. libgit2 only, and nothing is written: the trial merges happen in
//! memory, so no ref, index, working-tree file or object file changes.

use crate::branch_ref::BranchRef;
use crate::repo::Snapshot;
use git2::Oid;
use std::collections::HashMap;

mod forecast;
mod read;

pub use read::compute;

/// How many branches besides the main branch are measured.
pub const MAX_BRANCHES: usize = 40;
/// How many changed file names are kept per branch and per pair (the count stays exact; overlaps
/// are found on the whole lists).
pub const MAX_PATHS: usize = 5_000;

/// Two tips, the smaller object id first: the key a trial merge is remembered under.
pub type PairKey = (Oid, Oid);

pub fn pair_key(a: Oid, b: Oid) -> PairKey {
    if a <= b { (a, b) } else { (b, a) }
}

/// Trial merges already made, by the pair of tips they merged.
pub type Forecasts = HashMap<PairKey, Forecast>;

/// A branch and the commit it points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    pub branch: BranchRef,
    pub oid: Oid,
}

/// What to measure, planned from a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The main branch: the one every other branch is measured against.
    pub main: Tip,
    /// The other local branches that have a commit.
    pub branches: Vec<Tip>,
    /// The checked-out branch.
    pub current: Option<BranchRef>,
}

/// Why there is nothing to measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Empty {
    /// No local branch has a commit yet.
    NoCommits,
    /// The repository has no main branch.
    NoMain,
    /// Only the main branch exists.
    OneBranch,
}

/// What merging two branches would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Forecast {
    Clean,
    /// Would stop with conflicts on these paths.
    Conflicts(Vec<String>),
    /// The trial merge failed; libgit2's words.
    Unknown(String),
}

/// One measured branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub branch: BranchRef,
    pub tip: Oid,
    /// The tip's committer time in Unix seconds.
    pub time: i64,
    pub ahead: usize,
    pub behind: usize,
    /// The merge base with the main branch; None without shared history.
    pub base: Option<Oid>,
    /// The files changed from the merge base to the tip, sorted, at most `MAX_PATHS`.
    pub files: Vec<String>,
    pub files_total: usize,
}

/// Two branches that change a file in common.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    /// Row indexes, `a < b`.
    pub a: usize,
    pub b: usize,
    pub shared: Vec<String>,
    pub forecast: Forecast,
}

/// The measured branches and their pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchMap {
    /// `rows[0]` is the main branch, then the others, newest tip first.
    pub rows: Vec<Row>,
    pub pairs: Vec<Pair>,
    /// The checked-out branch's row.
    pub current: Option<usize>,
    /// How many other branches there were before the cap.
    pub total: usize,
    /// How many trial merges this read made (the others were known).
    pub merged: usize,
}

/// One block of the detail pane: another branch the selected one shares files with.
pub struct Shared<'a> {
    pub other: usize,
    pub forecast: &'a Forecast,
    /// Conflicting paths first (flagged), then the other shared paths.
    pub files: Vec<(&'a str, bool)>,
    pub shared: usize,
}

impl BranchMap {
    pub fn row_of(&self, b: &BranchRef) -> Option<usize> {
        self.rows.iter().position(|r| &r.branch == b)
    }

    /// The paths merging row `i` and the main branch would conflict on.
    pub fn conflicts_with_main(&self, i: usize) -> Option<&[String]> {
        let p = self.pairs.iter().find(|p| p.a == 0 && p.b == i)?;
        match &p.forecast {
            Forecast::Conflicts(paths) => Some(paths),
            _ => None,
        }
    }

    /// The blocks for row `i`: the main branch first, then those with conflicts, then by name.
    pub fn shared_with(&self, i: usize) -> Vec<Shared<'_>> {
        let mut out: Vec<Shared<'_>> = self
            .pairs
            .iter()
            .filter(|p| p.a == i || p.b == i)
            .map(|p| {
                let other = if p.a == i { p.b } else { p.a };
                let hit: &[String] = match &p.forecast {
                    Forecast::Conflicts(paths) => paths,
                    _ => &[],
                };
                let mut files: Vec<(&str, bool)> = hit.iter().map(|f| (f.as_str(), true)).collect();
                files.extend(p.shared.iter().filter(|f| !hit.contains(f)).map(|f| (f.as_str(), false)));
                Shared { other, forecast: &p.forecast, files, shared: p.shared.len() }
            })
            .collect();
        let rank = |s: &Shared<'_>| (s.other != 0, !matches!(s.forecast, Forecast::Conflicts(_)));
        out.sort_by(|x, y| rank(x).cmp(&rank(y)).then_with(|| self.rows[x.other].branch.shown().cmp(self.rows[y.other].branch.shown())));
        out
    }
}

/// The plan for `snap`: the main branch and the other branches with a commit, or why there is
/// nothing to measure. No repository is read.
pub fn request(snap: &Snapshot) -> Result<Request, Empty> {
    let tips: Vec<Tip> = snap.locals.iter().filter_map(|b| Some(Tip { branch: BranchRef::Local(b.name.clone()), oid: b.oid? })).collect();
    if tips.is_empty() {
        return Err(Empty::NoCommits);
    }
    let name = snap.mainline().ok_or(Empty::NoMain)?;
    let main = match snap.local_oid(&name) {
        Some(oid) => Tip { branch: BranchRef::Local(name), oid },
        None => {
            let names = || snap.remotes.iter().map(|(r, _)| r.as_str());
            let remotes = names().filter(|r| *r == "origin").chain(names().filter(|r| *r != "origin"));
            let (remote, oid) = remotes.into_iter().find_map(|r| Some((r, snap.remote_oid(r, &name)?))).ok_or(Empty::NoMain)?;
            Tip { branch: BranchRef::Remote(format!("{remote}/{name}")), oid }
        }
    };
    let branches: Vec<Tip> = tips.into_iter().filter(|t| t.branch != main.branch).collect();
    if branches.is_empty() {
        return Err(Empty::OneBranch);
    }
    let current = snap.head_oid.and(snap.head_branch.clone()).map(BranchRef::Local);
    Ok(Request { main, branches, current })
}

#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod tests;
