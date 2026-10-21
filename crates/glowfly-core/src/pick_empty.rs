//! Cherry-picks and reverts that stop because the commit they are at changes nothing (its
//! change is already on the branch, or was undone already): recognising the stop, and carrying
//! the run on past such commits with `--skip` (or ending a single pick with `--abort` on a git
//! without `--skip`).

use super::Picking;
use git2::{Oid, Repository};
use std::path::Path;

/// How a cherry-pick or revert job ended beyond git's exit status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickResult {
    pub picking: Picking,
    /// The commits the command took.
    pub total: usize,
    /// Commits that changed nothing and were skipped (or, for one revert, simply reported), in
    /// git's order.
    pub skipped: Vec<Oid>,
    /// A commit that changed nothing, left stopped in a sequence because git can't `--skip`.
    pub left_empty: Option<Oid>,
}

impl PickResult {
    /// The commits made, when the run succeeded.
    pub fn made(&self) -> usize {
        self.total.saturating_sub(self.skipped.len())
    }
}

/// How the lines git prints when the commit it is at would make no change begin (its output is
/// untranslated). Matching only line starts keeps the `[branch sha] subject` lines of commits
/// made earlier in the run from counting, whatever their subjects say.
const EMPTY_SIGNS: [&str; 5] = [
    "nothing to commit",
    "nothing added to commit",
    "no changes added to commit",
    "The previous cherry-pick is now empty",
    "The previous revert is now empty",
];

/// After a failed pick or revert that printed `output`: whether git stopped only because the
/// commit it was at changes nothing. The index has no conflicts, its tree is HEAD's, and a line
/// of git's own says so (the job log's `$ git` echo lines, `hint:` lines and commit subjects
/// don't count).
pub fn stopped_empty(dir: &Path, output: &str) -> bool {
    let said = output.lines().map(str::trim_start).any(|l| EMPTY_SIGNS.iter().any(|s| l.starts_with(s)));
    said && index_is_head(dir).unwrap_or(false)
}

fn index_is_head(dir: &Path) -> Result<bool, git2::Error> {
    let repo = Repository::open(dir)?;
    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Ok(false);
    }
    // Writes only a tree object, which a commit of the same changes would write anyway.
    let tree = index.write_tree()?;
    Ok(repo.head()?.peel_to_tree()?.id() == tree)
}

/// The commit a stopped run is at: the one git names in CHERRY_PICK_HEAD (or REVERT_HEAD) when
/// present, else `oids[applied + skipped]`, where `applied` is how many commits HEAD has
/// advanced since `start` (each pick or revert makes one ordinary commit). `None` when that is
/// out of range.
pub fn stopped_at(dir: &Path, picking: Picking, oids: &[Oid], start: Option<Oid>, skipped: usize) -> Option<Oid> {
    let repo = Repository::open(dir).ok()?;
    if let Some(o) = repo.find_reference(picking.head_ref()).ok().and_then(|r| r.target()) {
        return Some(o);
    }
    let head = repo.head().ok()?.target()?;
    let applied = match start {
        Some(s) => repo.graph_ahead_behind(head, s).ok()?.0,
        None => 0,
    };
    oids.get(applied + skipped).copied()
}

/// Whether a run is still stopped part way (a sequence, or one commit waiting to be committed),
/// and whether it is a sequence.
fn in_progress(dir: &Path, picking: Picking) -> (bool, bool) {
    let Ok(repo) = Repository::open(dir) else { return (false, false) };
    // The repository's own git dir, which a linked worktree has too.
    let sequence = repo.path().join("sequencer").is_dir();
    (sequence || repo.find_reference(picking.head_ref()).is_ok(), sequence)
}

/// Carries a failed pick or revert past the commits that change nothing. `first` is the
/// command's (ok, output); `start` is HEAD before it ran. `run` runs one git command line and
/// returns (ok, output as the job log shows it). Each such commit is skipped with `--skip`
/// (`can_skip`: git 2.23 or newer), at most once per commit; without it a single pick is
/// aborted and a sequence is left stopped at the commit (`left_empty`). A single revert that
/// changes nothing leaves no state behind, so it is only reported.
pub fn skip_empty(
    dir: &Path,
    picking: Picking,
    oids: &[Oid],
    start: Option<Oid>,
    first: (bool, String),
    can_skip: bool,
    run: &mut dyn FnMut(&[String]) -> (bool, String),
) -> (bool, String, PickResult) {
    let (mut ok, mut output) = first;
    let mut last = output.clone();
    let mut result = PickResult { picking, total: oids.len(), ..PickResult::default() };
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    while !ok && result.skipped.len() < oids.len() && stopped_empty(dir, &last) {
        let Some(at) = stopped_at(dir, picking, oids, start, result.skipped.len()) else { break };
        let (stopped, sequence) = in_progress(dir, picking);
        if !stopped {
            result.skipped.push(at);
            ok = true;
            break;
        }
        if !can_skip {
            if sequence {
                result.left_empty = Some(at);
            } else {
                let (aborted, out) = run(&args(picking.abort()));
                output.push('\n');
                output.push_str(&out);
                result.skipped.push(at);
                ok = aborted;
            }
            break;
        }
        result.skipped.push(at);
        let (skipped, out) = run(&args(picking.skip()));
        output.push('\n');
        output.push_str(&out);
        ok = skipped;
        last = out;
    }
    (ok, output, result)
}

#[cfg(test)]
#[path = "pick_empty_tests.rs"]
mod tests;
