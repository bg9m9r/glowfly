//! Applying a patch's changes with `git apply`: the command lines for the chosen options, and
//! reading what it said (the problems that stopped it, the conflicts a three-way fallback left,
//! the hunks saved to `.rej` files).

use crate::gitcli::args;

/// What to do when part of a patch doesn't apply.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum OnFail {
    /// Change nothing: a check runs first.
    #[default]
    Refuse,
    /// Merge through the index, leaving conflicts to resolve (`--3way`).
    ThreeWay,
    /// Apply what fits and save the rest next to each file as `<file>.rej` (`--reject`).
    Reject,
}

/// How to apply a patch's changes.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ApplyOptions {
    /// Stage the changes too (`--index`); implied by [`OnFail::ThreeWay`].
    pub index: bool,
    pub on_fail: OnFail,
    /// Take the changes out instead (`-R`).
    pub reverse: bool,
}

/// The commands that apply the patch file at `patch` as `o` says. With [`OnFail::Refuse`] a
/// check runs first, so nothing is written when any part doesn't apply.
pub fn apply_cmds(patch: &str, o: &ApplyOptions) -> Vec<Vec<String>> {
    let cmd = |head: &[&str], index: bool| {
        let mut c = args(head);
        if index {
            c.push("--index".into());
        }
        if o.reverse {
            c.push("-R".into());
        }
        c.push(patch.to_owned());
        c
    };
    match o.on_fail {
        OnFail::Refuse => vec![cmd(&["apply", "--check"], o.index), cmd(&["apply"], o.index)],
        OnFail::ThreeWay => vec![cmd(&["apply", "--3way"], false)],
        OnFail::Reject => vec![cmd(&["apply", "--reject"], o.index)],
    }
}

/// The files `git apply --reject` saved hunks for, with how many: its "Applying patch <f> with
/// <n> reject(s)..." lines.
pub fn rejects(output: &str) -> Vec<(String, u32)> {
    output
        .lines()
        .filter_map(|l| {
            let (file, rest) = l.trim_end().strip_prefix("Applying patch ")?.rsplit_once(" with ")?;
            let n = rest.split_once(" reject")?.0.parse().ok()?;
            Some((file.to_owned(), n))
        })
        .collect()
}

/// Where `git apply --reject` saves the hunks of `file` that didn't fit.
pub fn rej_name(file: &str) -> String {
    format!("{file}.rej")
}

/// The files a three-way apply left conflicted, in order and without duplicates: its "Applied
/// patch to '<f>' with conflicts." and "U <f>" lines.
pub fn three_way_conflicts(output: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in output.lines().map(str::trim_end) {
        let file = line
            .strip_prefix("Applied patch to '")
            .and_then(|r| r.strip_suffix("' with conflicts."))
            .or_else(|| line.strip_prefix("U ").filter(|f| !f.is_empty()));
        if let Some(f) = file.filter(|f| !out.iter().any(|o| o == f)) {
            out.push(f.to_owned());
        }
    }
    out
}

/// One reason `git apply` refused a patch.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyProblem {
    /// The file's lines around `line` no longer match a hunk.
    Hunk { file: String, line: u32 },
    /// The file doesn't match the patch (git's summary after the hunk problems).
    Mismatch { file: String },
    /// The patch changes a file the working tree doesn't have.
    Missing { file: String },
    /// The patch creates a file that already exists.
    Exists { file: String },
    /// Applying to the index too, the file's working copy differs from its index copy.
    Unstaged { file: String },
    /// The input holds no diff at all (empty, or not a patch).
    NoPatch,
    /// The patch file is malformed at `line`.
    Corrupt { line: u32 },
    /// Any other error, as git wrote it.
    Other(String),
}

impl ApplyProblem {
    /// A phrase for a sentence such as "x.patch doesn't apply: <phrase>."
    pub fn describe(&self) -> String {
        match self {
            ApplyProblem::Hunk { file, line } => format!("{file} has changed where the hunk at line {line} goes"),
            ApplyProblem::Mismatch { file } => format!("{file} doesn't match the patch"),
            ApplyProblem::Missing { file } => format!("{file} isn't in the working tree"),
            ApplyProblem::Exists { file } => format!("{file} already exists"),
            ApplyProblem::Unstaged { file } => format!("{file} has unstaged changes; stage them or apply to the working tree only"),
            ApplyProblem::NoPatch => "the file holds no changes git can apply".to_owned(),
            ApplyProblem::Corrupt { line } => format!("the patch is damaged at line {line}"),
            ApplyProblem::Other(text) => text.clone(),
        }
    }
}

/// The problems in `git apply`'s output, in order and without duplicates: only its `error:`
/// lines are read (command echoes and warnings are skipped). A file's "patch does not apply" is
/// dropped when a hunk problem already names that file.
pub fn apply_problems(output: &str) -> Vec<ApplyProblem> {
    let mut found: Vec<ApplyProblem> = Vec::new();
    for problem in output.lines().filter_map(|l| l.trim_end().strip_prefix("error: ")).map(parse_problem) {
        if !found.contains(&problem) {
            found.push(problem);
        }
    }
    let hunk_files: Vec<String> =
        found.iter().filter_map(|p| if let ApplyProblem::Hunk { file, .. } = p { Some(file.clone()) } else { None }).collect();
    found.retain(|p| !matches!(p, ApplyProblem::Mismatch { file } if hunk_files.contains(file)));
    found
}

/// One `error:` line's text (after the prefix) as a problem.
fn parse_problem(text: &str) -> ApplyProblem {
    let other = || ApplyProblem::Other(text.to_owned());
    if let Some(rest) = text.strip_prefix("patch failed: ") {
        return match rest.rsplit_once(':').map(|(f, l)| (f, l.trim().parse::<u32>())) {
            Some((file, Ok(line))) => ApplyProblem::Hunk { file: file.to_owned(), line },
            _ => other(),
        };
    }
    if let Some(file) = text.strip_suffix(": patch does not apply") {
        return ApplyProblem::Mismatch { file: file.to_owned() };
    }
    if let Some(file) = text.strip_suffix(": No such file or directory") {
        return ApplyProblem::Missing { file: file.to_owned() };
    }
    if let Some(file) = text.strip_suffix(": already exists in working directory") {
        return ApplyProblem::Exists { file: file.to_owned() };
    }
    if let Some(file) = text.strip_suffix(": does not match index") {
        return ApplyProblem::Unstaged { file: file.to_owned() };
    }
    if text.starts_with("No valid patches in input") {
        return ApplyProblem::NoPatch;
    }
    if let Some(line) = text.strip_prefix("corrupt patch at line ").and_then(|l| l.trim().parse().ok()) {
        return ApplyProblem::Corrupt { line };
    }
    other()
}

/// The first problem in `output`, plus how many more there are; None when it has no `error:`
/// line.
pub fn failure_summary(output: &str) -> Option<String> {
    let problems = apply_problems(output);
    let first = problems.first()?.describe();
    Some(match problems.len() - 1 {
        0 => first,
        more => format!("{first} (and {more} more; see the log)"),
    })
}
