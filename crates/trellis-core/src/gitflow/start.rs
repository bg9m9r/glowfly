//! Starting a feature, release or hotfix branch: the checks and the command line.

use super::{Branches, FlowKind, GitFlowSettings};
use crate::branch_ops::{NameProblem, check_branch_name};
use crate::gitcli::args;
use crate::tag_ops::{TagNameProblem, check_tag_name};

/// Why a branch can't be started.
#[derive(Debug, PartialEq)]
pub enum StartProblem {
    /// The full branch name's problem.
    Name(NameProblem, String),
    MissingBase {
        kind: FlowKind,
        base: String,
    },
    /// The existing tag the release or hotfix would be tagged as, possibly in different case.
    TagExists(String),
    /// The release or hotfix tag and an existing tag (the second name): one is a folder of the other.
    TagClashes(String, String),
}

impl StartProblem {
    pub fn message(&self) -> String {
        match self {
            StartProblem::Name(p, full) => p.message(full),
            StartProblem::MissingBase { kind: FlowKind::Hotfix, base } => {
                format!("{base} is needed to start a hotfix, and it doesn't exist.")
            }
            StartProblem::MissingBase { base, .. } => format!("{base} doesn't exist. Save the Git Flow settings to create it."),
            StartProblem::TagExists(t) => format!("The tag {t} already exists."),
            StartProblem::TagClashes(t, other) => format!("The tag {t} would clash with the tag {other}: one is a folder of the other."),
        }
    }
}

/// Checks a new `kind` branch named `name` (trimmed here); returns the full branch name.
pub fn check_start(s: &GitFlowSettings, kind: FlowKind, name: &str, b: &Branches) -> Result<String, StartProblem> {
    let name = name.trim();
    let full = format!("{}{name}", s.prefix(kind));
    if name.is_empty() {
        return Err(StartProblem::Name(NameProblem::Empty, full));
    }
    check_branch_name(&full, &b.locals, None).map_err(|p| StartProblem::Name(p, full.clone()))?;
    let base = s.base(kind);
    if !b.local(base) {
        return Err(StartProblem::MissingBase { kind, base: base.to_owned() });
    }
    // The tag Finish will suggest; an invalid one is left for Finish to report.
    let tag = format!("{}{name}", s.version_tag);
    if kind.versioned() {
        match check_tag_name(&tag, &b.tags) {
            Err(TagNameProblem::Exists(other)) => return Err(StartProblem::TagExists(other)),
            Err(TagNameProblem::Clashes(other)) => return Err(StartProblem::TagClashes(tag, other)),
            _ => {}
        }
    }
    Ok(full)
}

/// `git checkout -b <branch> refs/heads/<base>`: the full ref can't be mistaken for a tag.
pub fn start_cmd(s: &GitFlowSettings, kind: FlowKind, branch: &str) -> Vec<String> {
    args(&["checkout", "-b", branch, &format!("refs/heads/{}", s.base(kind))])
}

/// The local branches already named with `kind`'s prefix.
pub fn open_branches<'b>(s: &GitFlowSettings, kind: FlowKind, b: &Branches<'b>) -> Vec<&'b str> {
    let prefix = s.prefix(kind);
    b.locals.iter().copied().filter(|l| !prefix.is_empty() && l.starts_with(prefix)).collect()
}
