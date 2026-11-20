//! Building a commit index: the graph's history walk run to the end (or to the cap), reading what
//! search matches from each commit. Given the previous index of the same repository, commits it
//! already holds are copied from it instead of read again, so an update after a fetch or a commit
//! reads only the new ones.
//!
//! The invariant everything relies on: positions are the graph walk's pop order for `key.starts`
//! (the same queue, `repo::walk_order`, pushing the same parents in the same order), so
//! `repo::load` with `max_commits = p + 1` loads the commit at position `p`.
//!
//! An update stops walking once its queue equals the base walk's queue at some base position `p`
//! (the same commits, the same tie order, and exactly the base's first `p` commits visited). From
//! there the rest of the walk is the base's tail, which is appended as it is (`splice.rs`); when
//! no such point comes (a ref moved back, a force-push, a deleted branch with commits nothing
//! else reaches), the walk runs to the end.

use super::splice::Splice;
use super::{Builder, CommitIndex, IndexKey, MAX_INDEXED, Texts};
use crate::commit_text::Decoder;
use crate::error::Result;
use crate::repo::walk::flatten_body_into;
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

/// What an update did, for tests and the debug log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Commits read from the repository.
    pub read: usize,
    /// Base entries copied one by one through the queue.
    pub walked: usize,
    /// Base entries appended in bulk.
    pub spliced: usize,
    /// The base position the bulk copy started at.
    pub splice_at: Option<usize>,
}

pub(crate) fn build_paced(
    workdir: &Path,
    key: &IndexKey,
    base: Option<&CommitIndex>,
    progress: &mut dyn FnMut(Progress),
    stop: &dyn Fn() -> bool,
    pace: &Pace,
) -> Result<Option<CommitIndex>> {
    build_counted(workdir, key, base, progress, stop, pace).map(|o| o.map(|(ix, _)| ix))
}

/// `build_paced`, also saying what it read, walked and spliced.
pub(crate) fn build_counted(
    workdir: &Path,
    key: &IndexKey,
    base: Option<&CommitIndex>,
    progress: &mut dyn FnMut(Progress),
    stop: &dyn Fn() -> bool,
    pace: &Pace,
) -> Result<Option<(CommitIndex, Counts)>> {
    let _span = tracing::debug_span!("index.build", reuse = base.is_some()).entered();
    let repo = Repository::discover(workdir)?;
    // A capped base lacks parents of its oldest commits, and a deepened or unshallowed clone has
    // parents where the base found none.
    let base = base.filter(|b| b.complete() && b.key().shallow == key.shallow);
    let estimate = if base.is_none() { super::estimate::commit_count(repo.commondir()) } else { None };
    let report = |indexed: usize| Progress { indexed, estimate: estimate.filter(|&e| e >= indexed) };
    progress(report(0));

    let mut out = Builder::new(key.clone(), base);
    let mut counts = Counts::default();
    let mut splice = base.map(Splice::new);
    // A commit's committer time, and whether the base holds it.
    let time_in = |oid: Oid| -> Result<(i64, bool)> {
        match base.and_then(|b| b.time_of(oid)) {
            Some(t) => Ok((t, true)),
            None => Ok((repo.find_commit(oid)?.time().seconds(), false)),
        }
    };
    let mut queue = TimeQueue::default();
    for &start in &key.starts {
        let (t, known) = time_in(start)?;
        if queue.push(start, t)
            && let Some(s) = &mut splice
        {
            s.pushed(start, known);
        }
    }
    let mut last_report = Instant::now();
    let mut complete = true;
    let mut found = None;
    let mut parents: Vec<Oid> = Vec::new();
    // The flattened body, in a buffer reused for every commit.
    let mut body = String::new();
    loop {
        if let Some(p) = splice.as_mut().and_then(|s| s.ready(&queue)) {
            found = Some(p);
            break;
        }
        let Some(oid) = queue.pop() else { break };
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
        if let Some((pos, e, walked)) = base.and_then(|b| b.walked(oid)) {
            if let Some(s) = &mut splice {
                s.popped(oid, Some(pos));
            }
            for &p in walked {
                // The parents of a complete base's commit are in it.
                if queue.push(p, time_in(p)?.0)
                    && let Some(s) = &mut splice
                {
                    s.pushed(p, true);
                }
            }
            out.push_copied(e, walked);
            counts.walked += 1;
            continue;
        }
        if let Some(s) = &mut splice {
            s.popped(oid, None);
        }
        let c = repo.find_commit(oid)?;
        parents.clear();
        for (p, time) in walk_parents(&c) {
            if queue.push(p, time)
                && let Some(s) = &mut splice
            {
                s.pushed(p, base.is_some_and(|b| b.position(p).is_some()));
            }
            parents.push(p);
        }
        let d = Decoder::of(&c);
        body.clear();
        flatten_body_into(&d.body(&c), &mut body);
        let summary = d.summary(&c);
        let texts = Texts { summary: &summary, body: &body };
        let author = c.author();
        let (name, email) = (d.name(&author), d.email(&author));
        let who = (&*name, &*email);
        out.push_new(oid, (c.time().seconds(), author.when().seconds()), who, texts, &parents);
        counts.read += 1;
    }
    if let (Some(p), Some(b)) = (found, base) {
        // The rest of the walk is the base's tail from `p`: append it, as far as the cap allows.
        let total = b.len() - p;
        let take = total.min(pace.max.saturating_sub(out.len()));
        let at = out.len();
        let chunk = pace.check_every.max(1);
        let mut done = 0;
        while done < take {
            if stop() {
                return Ok(None);
            }
            let n = out.len();
            if n > 0 && last_report.elapsed() >= pace.report_every {
                progress(report(n));
                last_report = Instant::now();
            }
            let step = chunk.min(take - done);
            out.copy_tail(b, p + done, step);
            done += step;
        }
        out.index_tail(b, p, at, take);
        complete = take == total;
        counts.spliced = take;
        counts.splice_at = Some(p);
    }
    if stop() {
        return Ok(None);
    }
    tracing::debug!(read = counts.read, walked = counts.walked, spliced = counts.spliced, splice_at = ?counts.splice_at, "index.build");
    Ok(Some((out.finish(complete), counts)))
}
