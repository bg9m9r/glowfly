//! Shrinking a failing session: replaying its action list with chunks removed (halves, then
//! quarters, down to single actions) while the same rule still breaks, then making each kept
//! action simpler (shorter text, fewer lines). The predicate replays from a fresh scene, so this
//! file knows nothing about the app; replays of the next few chunks run side by side, and a
//! deadline keeps a failing run short.

use super::action::{Action, Outside};
use std::time::Instant;

/// The shortest sub-list of `actions` for which `breaks` still holds (ddmin: halves, then
/// quarters, down to single actions, keeping any shorter list that breaks), then each kept
/// action simplified by `simpler` (a shorter summary, fewer lines). Up to `width` lists are
/// tried at once; the first of them (in list order) that breaks is kept, so the result doesn't
/// depend on `width`. Stops at `deadline`, returning the best so far.
pub(super) fn shrink(actions: Vec<Action>, breaks: impl Fn(&[Action]) -> bool + Sync, deadline: Instant, width: usize) -> Vec<Action> {
    let mut best = actions;
    let time_left = || Instant::now() < deadline;
    let mut parts = 2;
    while best.len() >= 2 && time_left() {
        let chunk = best.len().div_ceil(parts);
        let mut removed = false;
        let mut start = 0;
        while start < best.len() && time_left() {
            // The lists without each of the next `width` chunks; the first that breaks is kept.
            let starts: Vec<usize> = (start..best.len()).step_by(chunk).take(width.max(1)).collect();
            let tries: Vec<Vec<Action>> =
                starts.iter().map(|&at| best[..at].iter().chain(&best[(at + chunk).min(best.len())..]).cloned().collect()).collect();
            match first_breaking(&tries, &breaks) {
                Some(k) => {
                    // The chunk after the removed one has moved up to its start.
                    start = starts[k];
                    best = tries.into_iter().nth(k).unwrap_or_default();
                    removed = true;
                }
                None => start = starts.last().map_or(best.len(), |at| at + chunk),
            }
        }
        if removed {
            parts = (parts - 1).max(2);
        } else if chunk == 1 {
            break;
        } else {
            parts = (parts * 2).min(best.len());
        }
    }
    for at in 0..best.len() {
        'simpler: while time_left() {
            for candidate in simpler(&best[at]) {
                if !time_left() {
                    break 'simpler;
                }
                let mut tried = best.clone();
                tried[at] = candidate;
                if breaks(&tried) {
                    best = tried;
                    continue 'simpler;
                }
            }
            break;
        }
    }
    best
}

/// The index of the first non-empty list in `tries` for which `breaks` holds, each tried on its
/// own thread.
fn first_breaking(tries: &[Vec<Action>], breaks: &(impl Fn(&[Action]) -> bool + Sync)) -> Option<usize> {
    if let [only] = tries {
        return (!only.is_empty() && breaks(only)).then_some(0);
    }
    std::thread::scope(|s| {
        let runs: Vec<_> = tries.iter().map(|t| s.spawn(move || !t.is_empty() && breaks(t))).collect();
        let held: Vec<bool> = runs.into_iter().map(|r| r.join().unwrap_or(false)).collect();
        held.iter().position(|b| *b)
    })
}

/// Simpler versions of `a` to try in its place, simplest first: a one-letter summary, then half
/// of it, and one line instead of several. Names (branches, paths, labels) are left alone: they
/// tie the action to what the session made.
pub(super) fn simpler(a: &Action) -> Vec<Action> {
    match a {
        Action::Commit(text) => shorter(text).into_iter().map(Action::Commit).collect(),
        Action::Outside(Outside::GitCommit(text)) => shorter(text).into_iter().map(|t| Action::Outside(Outside::GitCommit(t))).collect(),
        Action::Outside(o) => one_line(o).into_iter().map(Action::Outside).collect(),
        _ => Vec::new(),
    }
}

/// `c`, then the first half of `text`, each only when it is shorter than `text`.
fn shorter(text: &str) -> Vec<String> {
    let count = text.chars().count();
    let mut out = Vec::new();
    if count > 1 {
        out.push("c".to_owned());
    }
    if count / 2 > 1 {
        out.push(text.chars().take(count / 2).collect());
    }
    out
}

/// `o` adding one line instead of several.
fn one_line(o: &Outside) -> Option<Outside> {
    let path = match o {
        Outside::Write { path, lines } | Outside::Append { path, lines } | Outside::Create { path, lines } if *lines > 1 => path.clone(),
        _ => return None,
    };
    Some(match o {
        Outside::Write { .. } => Outside::Write { path, lines: 1 },
        Outside::Append { .. } => Outside::Append { path, lines: 1 },
        _ => Outside::Create { path, lines: 1 },
    })
}

#[cfg(test)]
#[path = "shrink_tests.rs"]
mod tests;
