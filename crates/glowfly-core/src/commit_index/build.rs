//! Building a commit index: the graph's history walk run to the end (or to the cap), reading what
//! search matches from each commit. Given the previous index of the same repository, commits it
//! already holds are copied from it instead of read again, so an update after a fetch or a commit
//! reads only the new ones.
//!
//! The invariant everything relies on: positions are the graph walk's pop order for `key.starts`
//! (the same queue, `repo::walk_order`, pushing the same parents in the same order), so
//! `repo::load` with `max_commits = p + 1` loads the commit at position `p`.

use super::{Builder, CommitIndex, IndexKey, MAX_INDEXED, Texts};
use crate::error::Result;
use crate::repo::walk::commit_texts;
use crate::repo::walk_order::{TimeQueue, walk_parents};
use git2::{Oid, Repository};
use std::path::Path;
use std::time::{Duration, Instant};

/// How far a build has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Commits indexed so far.
    pub indexed: usize,
    /// About how many commits there are (from git's commit-graph file), while `indexed` is below
    /// it; fresh builds only.
    pub estimate: Option<usize>,
}

/// How a build paces itself.
pub(crate) struct Pace {
    /// The most commits to index.
    pub max: usize,
    /// How many commits go by between looks at `stop` and the clock.
    pub check_every: usize,
    /// The least time between progress reports.
    pub report_every: Duration,
}

impl Pace {
    pub(crate) fn capped(max: usize) -> Pace {
        Pace { max, check_every: 1024, report_every: Duration::from_millis(250) }
    }
}

/// Indexes the history of the repository at `workdir` walked from `key` (see the module docs).
/// `base`, the tab's previous index, is reused when it is complete and has the same shallow
/// boundary; otherwise every commit is read. `progress` hears how far it got now and then;
/// `Ok(None)` when `stop` said to stop.
pub fn build(
    workdir: &Path,
    key: &IndexKey,
    base: Option<&CommitIndex>,
    progress: &mut dyn FnMut(Progress),
    stop: &dyn Fn() -> bool,
) -> Result<Option<CommitIndex>> {
    build_capped(workdir, key, base, progress, stop, MAX_INDEXED)
}

/// `build`, holding at most `max` commits.
pub(crate) fn build_capped(
    workdir: &Path,
    key: &IndexKey,
    base: Option<&CommitIndex>,
    progress: &mut dyn FnMut(Progress),
    stop: &dyn Fn() -> bool,
    max: usize,
) -> Result<Option<CommitIndex>> {
    build_paced(workdir, key, base, progress, stop, &Pace::capped(max))
}

pub(crate) fn build_paced(
    workdir: &Path,
    key: &IndexKey,
    base: Option<&CommitIndex>,
    progress: &mut dyn FnMut(Progress),
    stop: &dyn Fn() -> bool,
    pace: &Pace,
) -> Result<Option<CommitIndex>> {
    let _span = tracing::debug_span!("index.build", reuse = base.is_some()).entered();
    let repo = Repository::discover(workdir)?;
    // A capped base lacks parents of its oldest commits, and a deepened or unshallowed clone has
    // parents where the base found none.
    let base = base.filter(|b| b.complete() && b.key().shallow == key.shallow);
    let estimate = if base.is_none() { super::estimate::commit_count(repo.commondir()) } else { None };
    let report = |indexed: usize| Progress { indexed, estimate: estimate.filter(|&e| e >= indexed) };
    progress(report(0));

    let mut out = Builder::new(key.clone(), base);
    let time_of = |oid: Oid| -> Result<i64> {
        match base.and_then(|b| b.time_of(oid)) {
            Some(t) => Ok(t),
            None => Ok(repo.find_commit(oid)?.time().seconds()),
        }
    };
    let mut queue = TimeQueue::default();
    for &start in &key.starts {
        queue.push(start, time_of(start)?);
    }
    let mut last_report = Instant::now();
    let mut complete = true;
    let mut parents: Vec<Oid> = Vec::new();
    while let Some(oid) = queue.pop() {
        let n = out.len();
        if n >= pace.max {
            complete = false;
            break;
        }
        if n.is_multiple_of(pace.check_every.max(1)) {
            if stop() {
                return Ok(None);
            }
            if n > 0 && last_report.elapsed() >= pace.report_every {
                progress(report(n));
                last_report = Instant::now();
            }
        }
        if let Some((e, walked)) = base.and_then(|b| b.walked(oid)) {
            for &p in walked {
                queue.push(p, time_of(p)?);
            }
            out.push_copied(e, walked);
            continue;
        }
        let c = repo.find_commit(oid)?;
        parents.clear();
        for (p, time) in walk_parents(&c) {
            queue.push(p, time);
            parents.push(p);
        }
        let (summary, body) = commit_texts(&c);
        let author = c.author();
        let who = (author.name().unwrap_or_default(), author.email().unwrap_or_default());
        out.push_new(oid, (c.time().seconds(), author.when().seconds()), who, Texts { summary: &summary, body: &body }, &parents);
    }
    if stop() {
        return Ok(None);
    }
    Ok(Some(out.finish(complete)))
}
