//! Finishing a feature, release or hotfix branch: which local branches are flow branches, the
//! checks against the snapshot, and the plan of git commands (merges with --no-ff, the release
//! or hotfix tag, and deleting the branch). Pure: no repository access.

use super::{Branches, FlowKind, GitFlowSettings};
use crate::gitcli::args;
use crate::rebase::operation_noun;
use crate::repo::Snapshot;
use git2::RepositoryState;

/// The kind of a local branch from the configured prefixes: the longest matching prefix wins
/// (config written by other tools may overlap); None for production/development, for a name
/// that is only the prefix, or no match. Case-sensitive like `open_branches`.
pub fn flow_kind(s: &GitFlowSettings, branch: &str) -> Option<FlowKind> {
    if branch == s.production || branch == s.development {
        return None;
    }
    FlowKind::ALL
        .into_iter()
        .filter(|k| {
            let p = s.prefix(*k);
            !p.is_empty() && branch.len() > p.len() && branch.starts_with(p)
        })
        .max_by_key(|k| s.prefix(*k).len())
}

/// The name after the prefix ("login", "1.2.0").
pub fn short_name<'a>(s: &GitFlowSettings, kind: FlowKind, branch: &'a str) -> &'a str {
    branch.strip_prefix(s.prefix(kind)).unwrap_or(branch)
}

/// Every local flow branch with its kind: HEAD's first, then snapshot order.
pub fn flow_branches<'b>(s: &GitFlowSettings, b: &Branches<'b>) -> Vec<(FlowKind, &'b str)> {
    let mut out: Vec<(FlowKind, &'b str)> = b.locals.iter().filter_map(|l| flow_kind(s, l).map(|k| (k, *l))).collect();
    if let Some(i) = out.iter().position(|(_, l)| Some(*l) == b.head) {
        let head = out.remove(i);
        out.insert(0, head);
    }
    out
}

/// "<versiontag><short>" for a release or hotfix, None for a feature.
pub fn default_tag(s: &GitFlowSettings, kind: FlowKind, branch: &str) -> Option<String> {
    kind.versioned().then(|| format!("{}{}", s.version_tag, short_name(s, kind, branch)))
}

/// "Release 1.2.0" / "Hotfix 1.2.1" ("" for a feature).
pub fn default_tag_message(kind: FlowKind, short: &str) -> String {
    match kind {
        FlowKind::Feature => String::new(),
        FlowKind::Release => format!("Release {short}"),
        FlowKind::Hotfix => format!("Hotfix {short}"),
    }
}

/// "develop" or "main and develop": the branches a finished `kind` is merged into.
pub fn targets(s: &GitFlowSettings, kind: FlowKind) -> String {
    if kind.versioned() { format!("{} and {}", s.production, s.development) } else { s.development.clone() }
}

/// What else Finish checks in the repository.
pub struct WorkState<'a> {
    pub state: RepositoryState,
    /// Staged or unstaged changes to tracked files (untracked '?' entries don't count).
    pub tracked_changes: bool,
    /// Branches checked out in the other worktrees.
    pub elsewhere: Vec<&'a str>,
}

impl<'a> WorkState<'a> {
    pub fn of(snap: &'a Snapshot) -> Self {
        WorkState {
            state: snap.state,
            tracked_changes: !snap.staged.is_empty() || snap.unstaged.iter().any(|f| f.status != '?'),
            elsewhere: snap.worktrees.iter().filter(|w| !w.current).filter_map(|w| w.branch.as_deref()).collect(),
        }
    }
}

/// The Finish form's choices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FinishOptions {
    /// The tag name (release and hotfix only).
    pub tag: String,
    /// The annotated tag's message (release and hotfix only).
    pub message: String,
    /// Keep the branch after merging it.
    pub keep: bool,
}

/// One git command of the plan.
#[derive(Debug, PartialEq)]
pub struct FinishStep {
    pub cmd: Vec<String>,
    /// "merge feature/x into develop" (lower case).
    pub what: String,
}

/// What Finish will do.
#[derive(Debug, PartialEq)]
pub struct FinishPlan {
    pub kind: FlowKind,
    pub branch: String,
    pub short: String,
    /// The tag made on production (release and hotfix).
    pub tag: Option<String>,
    pub keep: bool,
    pub steps: Vec<FinishStep>,
}

impl FinishPlan {
    pub fn cmds(&self) -> Vec<Vec<String>> {
        self.steps.iter().map(|s| s.cmd.clone()).collect()
    }

    /// "Finish feature login".
    pub fn label(&self) -> String {
        format!("Finish {} {}", self.kind.noun(), self.short)
    }

    /// "Finished release 1.2.0: merged into main and develop, tagged v1.2.0 and deleted
    /// release/1.2.0."
    pub fn summary(&self, s: &GitFlowSettings) -> String {
        let mut done = vec![format!("merged into {}", targets(s, self.kind))];
        if let Some(t) = &self.tag {
            done.push(format!("tagged {t}"));
        }
        if !self.keep {
            done.push(format!("deleted {}", self.branch));
        }
        let last = done.pop().unwrap_or_default();
        let text = if done.is_empty() { last } else { format!("{} and {last}", done.join(", ")) };
        format!("Finished {} {}: {text}.", self.kind.noun(), self.short)
    }
}

/// Why a branch can't be finished.
#[derive(Debug, PartialEq)]
pub enum FinishProblem {
    NotFlowBranch(String),
    NoBranch(String),
    /// The production and/or development branch the kind merges into is missing.
    MissingBases {
        kind: FlowKind,
        missing: Vec<String>,
    },
    /// The noun of the operation in progress ("merge", "rebase"…).
    InProgress(&'static str),
    /// The branch being finished.
    TrackedChanges(String),
    CheckedOutElsewhere(String),
    EmptyTag,
    InvalidTag(String),
    TagExists(String),
    EmptyMessage,
}

impl FinishProblem {
    pub fn message(&self) -> String {
        use FinishProblem as P;
        match self {
            P::NotFlowBranch(b) => format!("{b} isn't a feature, release or hotfix branch."),
            P::NoBranch(b) => format!("{b} doesn't exist."),
            P::MissingBases { kind, missing } => {
                let end = if missing.len() == 1 { "and it doesn't exist." } else { "and neither exists." };
                format!("Finishing a {} needs {}, {end}", kind.noun(), missing.join(" and "))
            }
            P::InProgress(op) => format!("Finish or abort the {op} in progress first."),
            P::TrackedChanges(b) => format!("Commit or stash your changes before finishing {b}."),
            P::CheckedOutElsewhere(b) => format!("{b} is checked out in another worktree. Switch that worktree to another branch first."),
            P::EmptyTag => "Enter a tag name.".into(),
            P::InvalidTag(t) => format!("{t} isn't a valid tag name."),
            P::TagExists(t) => format!("The tag {t} already exists."),
            P::EmptyMessage => "Enter a tag message.".into(),
        }
    }
}

/// Checks in this order: flow branch, exists locally, bases exist, no operation in progress,
/// no tracked changes, nothing needed checked out elsewhere, then (release/hotfix) the tag
/// (trimmed: non-empty, a valid tag name not starting with '-', not taken) and the message
/// (trimmed, non-empty). Returns the plan.
pub fn check_finish(
    s: &GitFlowSettings,
    branch: &str,
    o: &FinishOptions,
    b: &Branches,
    w: &WorkState,
) -> Result<FinishPlan, FinishProblem> {
    use FinishProblem as P;
    let kind = flow_kind(s, branch).ok_or_else(|| P::NotFlowBranch(branch.to_owned()))?;
    if !b.local(branch) {
        return Err(P::NoBranch(branch.to_owned()));
    }
    let bases: Vec<&str> = if kind.versioned() { vec![&s.production, &s.development] } else { vec![&s.development] };
    let missing: Vec<String> = bases.iter().filter(|n| !b.local(n)).map(|n| n.to_string()).collect();
    if !missing.is_empty() {
        return Err(P::MissingBases { kind, missing });
    }
    if let Some(op) = operation_noun(w.state) {
        return Err(P::InProgress(op));
    }
    if w.tracked_changes {
        return Err(P::TrackedChanges(branch.to_owned()));
    }
    let deleted = (!o.keep).then_some(branch);
    if let Some(n) = bases.iter().copied().chain(deleted).find(|n| w.elsewhere.contains(n)) {
        return Err(P::CheckedOutElsewhere(n.to_owned()));
    }
    let tag = if kind.versioned() {
        let (tag, message) = (o.tag.trim(), o.message.trim());
        if tag.is_empty() {
            return Err(P::EmptyTag);
        }
        if tag.starts_with('-') || !git2::Reference::is_valid_name(&format!("refs/tags/{tag}")) {
            return Err(P::InvalidTag(tag.to_owned()));
        }
        if b.tags.contains(&tag) {
            return Err(P::TagExists(tag.to_owned()));
        }
        if message.is_empty() {
            return Err(P::EmptyMessage);
        }
        Some((tag.to_owned(), message.to_owned()))
    } else {
        None
    };
    let steps = steps(s, kind, branch, tag.as_ref(), o.keep);
    let short = short_name(s, kind, branch).to_owned();
    Ok(FinishPlan { kind, branch: branch.to_owned(), short, tag: tag.map(|(t, _)| t), keep: o.keep, steps })
}

/// The commands. Merges name refs/heads/ so a tag or path with the same name can't be picked;
/// checkouts end with `--` so the name can't be read as a path.
fn steps(s: &GitFlowSettings, kind: FlowKind, branch: &str, tag: Option<&(String, String)>, keep: bool) -> Vec<FinishStep> {
    let step = |cmd: &[&str], what: String| FinishStep { cmd: args(cmd), what };
    let full = format!("refs/heads/{branch}");
    let checkout = |name: &str| step(&["checkout", name, "--"], format!("check out {name}"));
    let merge = |into: &str| step(&["merge", "--no-ff", "--no-edit", &full], format!("merge {branch} into {into}"));
    let (p, d) = (s.production.as_str(), s.development.as_str());
    let mut out = Vec::new();
    if let (true, Some((t, m))) = (kind.versioned(), tag) {
        out.push(checkout(p));
        out.push(merge(p));
        out.push(step(&["tag", "-a", t, "-m", m, &format!("refs/heads/{p}")], format!("tag {p} as {t}")));
    }
    out.push(checkout(d));
    out.push(merge(d));
    if !keep {
        out.push(step(&["branch", "-d", branch], format!("delete {branch}")));
    }
    out
}
