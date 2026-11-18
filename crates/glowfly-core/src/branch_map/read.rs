//! Measuring the branches of a `Request`: ahead and behind the main branch, the merge base, the
//! files changed since it, which pairs share a file, and a trial merge for each such pair that
//! no earlier read has made (`Forecasts`). Reads the repository through its own handle.

use super::forecast::Merger;
use super::{BranchMap, Forecast, Forecasts, MAX_BRANCHES, MAX_PATHS, Pair, Request, Row, Tip, pair_key};
use crate::error::Result;
use crate::range_diff::bases::merge_base;
use git2::{DiffOptions, Oid, Repository};
use std::cmp::Reverse;
use std::path::Path;

/// The map for `req`. `known` are trial merges already made; `stop` is asked between the steps
/// and gives `Ok(None)` when it says so.
pub fn compute(workdir: &Path, req: &Request, known: &Forecasts, stop: &dyn Fn() -> bool) -> Result<Option<BranchMap>> {
    let repo = Repository::open(workdir)?;
    let odb = repo.odb()?;
    let merger = Merger::new(&repo, &odb)?;
    let kept = kept(&repo, req)?;
    let main_time = repo.find_commit(req.main.oid)?.time().seconds();
    let mut rows = vec![Row {
        branch: req.main.branch.clone(),
        tip: req.main.oid,
        time: main_time,
        ahead: 0,
        behind: 0,
        base: Some(req.main.oid),
        files: Vec::new(),
        files_total: 0,
    }];
    // The whole sorted lists of changed files, for the overlaps (a row keeps only `MAX_PATHS`), and
    // the files each branch's merge base to the main branch's tip changed, for the main pairs.
    let mut whole: Vec<Vec<String>> = vec![Vec::new()];
    let mut main_sides: Vec<Vec<String>> = vec![Vec::new()];
    for (tip, time) in kept {
        if stop() {
            return Ok(None);
        }
        let (ahead, behind) = repo.graph_ahead_behind(tip.oid, req.main.oid)?;
        let base = merge_base(&repo, tip.oid, req.main.oid)?;
        let (all_files, files_total) = match base.filter(|b| *b != tip.oid) {
            Some(b) => changed(&repo, b, tip.oid)?,
            None => (Vec::new(), 0),
        };
        let side = match base.filter(|_| ahead > 0 && behind > 0) {
            Some(b) => changed(&repo, b, req.main.oid)?.0,
            None => Vec::new(),
        };
        main_sides.push(side);
        let files = all_files[..all_files.len().min(MAX_PATHS)].to_vec();
        whole.push(all_files);
        rows.push(Row { branch: tip.branch, tip: tip.oid, time, ahead, behind, base, files, files_total });
    }
    let current = req.current.as_ref().and_then(|c| rows.iter().position(|r| &r.branch == c));
    let mut pairs = Vec::new();
    let mut merged = 0;
    for a in 0..rows.len() {
        for b in a + 1..rows.len() {
            let ours = if a == 0 { &main_sides[b] } else { &whole[a] };
            let mut shared = common(ours, &whole[b]);
            if shared.is_empty() {
                continue;
            }
            if stop() {
                return Ok(None);
            }
            let key = pair_key(rows[a].tip, rows[b].tip);
            let forecast = match known.get(&key) {
                Some(f) => f.clone(),
                None if key.0 == key.1 => Forecast::Clean,
                None => {
                    merged += 1;
                    merger.forecast(key.0, key.1).unwrap_or_else(|e| Forecast::Unknown(e.to_string()))
                }
            };
            shared.truncate(MAX_PATHS);
            pairs.push(Pair { a, b, shared, forecast });
        }
    }
    Ok(Some(BranchMap { rows, pairs, current, total: req.branches.len(), merged }))
}

/// The branches to measure: the `MAX_BRANCHES` most recently committed (ties by name), the
/// checked-out one always among them; each with its tip's committer time.
fn kept(repo: &Repository, req: &Request) -> Result<Vec<(Tip, i64)>> {
    let mut all = Vec::with_capacity(req.branches.len());
    for t in &req.branches {
        all.push((t.clone(), repo.find_commit(t.oid)?.time().seconds()));
    }
    all.sort_by(|x, y| (Reverse(x.1), x.0.branch.shown()).cmp(&(Reverse(y.1), y.0.branch.shown())));
    if all.len() > MAX_BRANCHES {
        let current = req.current.as_ref().and_then(|c| all.iter().position(|(t, _)| &t.branch == c));
        if let Some(i) = current.filter(|i| *i >= MAX_BRANCHES) {
            all.swap(MAX_BRANCHES - 1, i);
        }
        all.truncate(MAX_BRANCHES);
        all.sort_by(|x, y| (Reverse(x.1), x.0.branch.shown()).cmp(&(Reverse(y.1), y.0.branch.shown())));
    }
    Ok(all)
}

/// The names of the files changed from `from`'s tree to `to`'s (a rename is its old and its new
/// name: no rename detection), sorted and whole (callers cut what they store), and how many deltas
/// there were.
fn changed(repo: &Repository, from: Oid, to: Oid) -> Result<(Vec<String>, usize)> {
    let (old, new) = (repo.find_commit(from)?.tree()?, repo.find_commit(to)?.tree()?);
    let mut opts = DiffOptions::new();
    opts.skip_binary_check(true);
    let diff = repo.diff_tree_to_tree(Some(&old), Some(&new), Some(&mut opts))?;
    let mut names: Vec<String> = diff
        .deltas()
        .filter_map(|d| d.new_file().path_bytes().or_else(|| d.old_file().path_bytes()))
        .map(crate::repo::shown_path)
        .collect();
    let total = diff.deltas().len();
    names.sort();
    names.dedup();
    Ok((names, total))
}

/// The names in both sorted lists.
fn common(a: &[String], b: &[String]) -> Vec<String> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        match a[i].as_bytes().cmp(b[j].as_bytes()) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i].clone());
                i += 1;
                j += 1;
            }
        }
    }
    out
}
