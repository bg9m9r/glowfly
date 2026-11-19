//! Apply and Unstash of a listed stash with its staged state: `git stash pop|apply --index`
//! brings back what was staged when the stash was made, so the stash comes back as it went.
//!
//! Two cases need care. First, git applies a stash's index part by resetting the index to HEAD
//! and merging, so with anything staged `--index` fails ("Your local changes ... would be
//! overwritten by merge") and loses the staged selection, where the plain form succeeds and
//! keeps it: `--index` is only used when nothing is staged (`anything_staged`). Second, when the
//! stash's staged part no longer fits the index git refuses with "Try without --index" and
//! changes nothing; `retry_unstaged` then runs the plain form once, and the output ends with
//! `BACK_UNSTAGED` so the job can say the changes came back unstaged. A conflict in the working
//! tree part, or any other failure, runs nothing more and keeps the stash.

use super::auto::{conflicted_paths, logged};
use super::list;
use git2::Oid;
use std::path::Path;

/// The last line of the log when the stash came back unstaged because git couldn't restore the
/// staged state (fixed English text, as the auto-stash message is; the toast is translated).
pub const BACK_UNSTAGED: &str = "Git couldn't restore what was staged, so the stash's changes are back unstaged.";

/// The command applying (or popping) `stash@{index}`, with `--index` when `with_index`.
pub fn apply_command(pop: bool, index: usize, with_index: bool) -> Vec<String> {
    let mut cmd = vec!["stash".to_owned(), if pop { "pop" } else { "apply" }.to_owned()];
    if with_index {
        cmd.push("--index".to_owned());
    }
    cmd.push(format!("stash@{{{index}}}"));
    cmd
}

/// Whether anything is staged (the index differs from HEAD). Any doubt, such as a folder that
/// isn't a repository, counts as staged: the plain form is the safe one.
pub fn anything_staged(workdir: &Path) -> bool {
    let staged = || -> Result<bool, git2::Error> {
        let repo = git2::Repository::open(workdir)?;
        let head = match repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e),
        };
        Ok(repo.diff_tree_to_index(head.as_ref(), None, None)?.deltas().len() > 0)
    };
    staged().unwrap_or(true)
}

/// Whether git's `output` is its refusal to restore the index ("Try without --index").
pub(super) fn index_refusal(output: &str) -> bool {
    output.to_lowercase().contains("without --index")
}

/// After a failed `--index` run with `output`: runs the `plain` command once through `run` when
/// git refused the staged state, nothing is conflicted and (with `stash`) the position the
/// command names still holds that commit. Returns whether it worked and both outputs joined,
/// ending with `BACK_UNSTAGED` when it did; `None` (nothing run) when the failure is any other.
pub fn retry_unstaged(
    workdir: &Path,
    output: &str,
    plain: &[String],
    stash: Option<Oid>,
    run: &mut dyn FnMut(&[String]) -> (bool, String),
) -> Option<(bool, String)> {
    if !index_refusal(output) || !conflicted_paths(workdir).is_empty() {
        return None;
    }
    if let Some(oid) = stash {
        let repo = git2::Repository::open(workdir).ok()?;
        let at = repo.revparse_single(plain.last()?).ok()?;
        if at.id() != oid {
            return None;
        }
    }
    let (ok, text) = run(plain);
    let mut joined = format!("{output}\n{text}");
    if ok {
        joined.push('\n');
        joined.push_str(BACK_UNSTAGED);
    }
    Some((ok, joined.trim().to_owned()))
}

/// Whether a job's `output` ends with the note that the stash came back unstaged.
pub fn came_back_unstaged(output: &str) -> bool {
    output.trim_end().ends_with(BACK_UNSTAGED)
}

/// How an Apply or Unstash went.
#[derive(Clone, Debug, PartialEq)]
pub enum StashBack {
    /// With what was staged, staged again.
    Staged,
    /// Git refused the staged state; the plain run worked and everything is unstaged.
    Unstaged,
    /// Something was staged, so it ran without `--index` and worked.
    Plain,
    /// Failed with these conflicted paths; the entry is kept.
    Conflicted { paths: Vec<String> },
    /// Failed some other way; the entry is kept.
    Failed,
}

impl StashBack {
    /// Whether the stash's changes are back.
    pub fn is_back(&self) -> bool {
        matches!(self, StashBack::Staged | StashBack::Unstaged | StashBack::Plain)
    }
}

/// Applies (or pops) `stash@{index}`, which must still be commit `oid`, with its staged state
/// where that is possible (the module docs), running each git command through `run` (whether it
/// worked, and the command with its output). The text is the commands' output for the log.
pub fn apply_with(
    workdir: &Path,
    pop: bool,
    index: usize,
    oid: Oid,
    run: &mut dyn FnMut(&[String]) -> (bool, String),
) -> (StashBack, String) {
    match list(workdir) {
        Err(e) => return (StashBack::Failed, format!("Could not read the stash list: {e}")),
        Ok(l) if l.get(index).map(|s| s.oid) != Some(oid) => {
            return (StashBack::Failed, format!("stash@{{{index}}} is no longer the stash that was picked; nothing was changed."));
        }
        Ok(_) => {}
    }
    let with_index = !anything_staged(workdir);
    let (mut ok, mut output) = run(&apply_command(pop, index, with_index));
    let mut fell_back = false;
    if !ok
        && with_index
        && let Some((again, text)) = retry_unstaged(workdir, &output, &apply_command(pop, index, false), Some(oid), run)
    {
        (ok, output, fell_back) = (again, text, true);
    }
    let back = match (ok, fell_back, with_index) {
        (true, true, _) => StashBack::Unstaged,
        (true, false, true) => StashBack::Staged,
        (true, false, false) => StashBack::Plain,
        (false, ..) => match conflicted_paths(workdir) {
            paths if paths.is_empty() => StashBack::Failed,
            paths => StashBack::Conflicted { paths },
        },
    };
    (back, output)
}

/// `apply_with`, running git directly.
pub fn apply(workdir: &Path, pop: bool, index: usize, oid: Oid) -> (StashBack, String) {
    apply_with(workdir, pop, index, oid, &mut |cmd| logged(workdir, cmd))
}
