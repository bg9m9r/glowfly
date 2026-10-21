//! Predicting merge conflicts with `git merge-tree --write-tree`, run through `gitcli` so the
//! user's merge config and the repository's merge drivers apply. It never touches the working
//! tree, the index or any ref: merge-tree only writes unreferenced objects, which gc removes.
//!
//! Also choosing the branch to compare the checked-out branch with ([`target_for`]), from a
//! loaded [`Snapshot`] alone.

use crate::error::{Error, Result};
use crate::gitcli::{self, GitOutput};
use crate::repo::{BranchInfo, Snapshot};
use crate::text::printable;
use git2::Oid;
use std::path::Path;
use std::time::Duration;

#[cfg(test)]
mod target_tests;
#[cfg(test)]
mod tests;

/// The oldest git whose merge-tree has `--write-tree`.
pub const MIN_GIT: (u32, u32) = (2, 38);

/// How long one prediction may take before git is stopped.
pub const PREDICT_TIMEOUT: Duration = Duration::from_secs(120);

/// Where a merge target came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// The branch's upstream, which has commits the branch lacks.
    Upstream,
    /// The repository's default branch (the remote's HEAD, main or master).
    DefaultBranch,
    /// A branch the user picked.
    Chosen,
}

/// The branch a check merges into the checked-out one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeTarget {
    /// A short ref name git accepts: "origin/main" or "main".
    pub name: String,
    pub oid: Oid,
    pub kind: TargetKind,
}

/// What merging the target would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prediction {
    /// The branch already contains the target.
    UpToDate,
    /// The branch has nothing the target lacks: merging moves it forward.
    FastForward,
    /// A real merge, without conflicts.
    Clean,
    /// A real merge that stops with conflicts in these paths (git's order, each once).
    Conflicts(Vec<String>),
}

/// The branch to compare the checked-out branch with, or None (detached or unborn HEAD, or
/// nothing suitable):
/// 1. its upstream, when that exists and has commits the branch lacks;
/// 2. otherwise the default branch: on the upstream's remote (else `origin`, else the first
///    remote) its recorded HEAD, then `main`, then `master`; then the local `main` and
///    `master`. The first that exists and is neither the branch itself nor its upstream wins.
pub fn target_for(snap: &Snapshot) -> Option<MergeTarget> {
    let branch = snap.head_branch.as_deref()?;
    snap.head_oid?;
    let head = snap.head_info()?;
    let upstream = head.upstream.as_deref();
    if let (Some(name), Some(oid)) = (upstream, snap.upstream_oid(head))
        && head.behind > 0
    {
        return Some(MergeTarget { name: name.to_owned(), oid, kind: TargetKind::Upstream });
    }
    let remote = home_remote(snap, head);
    let mut candidates: Vec<(String, Option<Oid>)> = Vec::new();
    if let Some(r) = remote {
        let recorded = snap.remote_heads.iter().find(|(name, _)| name == r).map(|(_, b)| b.as_str());
        for b in recorded.into_iter().chain(["main", "master"]) {
            candidates.push((format!("{r}/{b}"), snap.remote_oid(r, b)));
        }
    }
    for b in ["main", "master"] {
        candidates.push((b.to_owned(), snap.local_oid(b)));
    }
    candidates.into_iter().find_map(|(name, oid)| {
        let usable = name != branch && Some(name.as_str()) != upstream;
        usable.then_some(MergeTarget { name, oid: oid?, kind: TargetKind::DefaultBranch })
    })
}

/// The remote whose default branch counts: the upstream's, else `origin`, else the first one.
fn home_remote<'a>(snap: &'a Snapshot, head: &BranchInfo) -> Option<&'a str> {
    let names = || snap.remotes.iter().map(|(r, _)| r.as_str());
    let tracked = head.tracking.as_ref().filter(|t| t.is_remote()).map(|t| t.remote.as_str());
    tracked.and_then(|t| names().find(|r| *r == t)).or_else(|| names().find(|r| *r == "origin")).or_else(|| names().next())
}

fn too_old(version: Option<(u32, u32, u32)>) -> Error {
    let (major, minor) = MIN_GIT;
    let mut text = format!("Predicting conflicts needs git {major}.{minor} or newer");
    match version {
        Some((a, b, c)) => text.push_str(&format!("; this is git {a}.{b}.{c}.")),
        None => text.push('.'),
    }
    Error::Unsupported(text)
}

/// What merging `theirs` into `ours` in the repository at `workdir` would do. Runs
/// `git merge-tree --write-tree --name-only --no-messages -z <ours> <theirs>` (oids only, so
/// no name can be read as an option), never prompting and stopped after [`PREDICT_TIMEOUT`].
/// Commits that contain one another are answered from the graph without running merge-tree.
pub fn predict(workdir: &Path, ours: Oid, theirs: Oid) -> Result<Prediction> {
    if let Some(v) = gitcli::version(workdir)
        && (v.0, v.1) < MIN_GIT
    {
        return Err(too_old(Some(v)));
    }
    let repo = git2::Repository::open(workdir)?;
    if ours == theirs || repo.graph_descendant_of(ours, theirs)? {
        return Ok(Prediction::UpToDate);
    }
    if repo.graph_descendant_of(theirs, ours)? {
        return Ok(Prediction::FastForward);
    }
    if let Err(e) = repo.merge_base(ours, theirs) {
        if e.code() == git2::ErrorCode::NotFound {
            return Err(Error::Unsupported("the two branches share no history".into()));
        }
        return Err(e.into());
    }
    let (ours, theirs) = (ours.to_string(), theirs.to_string());
    let args = gitcli::args(&["merge-tree", "--write-tree", "--name-only", "--no-messages", "-z", &ours, &theirs]);
    let out = gitcli::output(workdir, &args, "", gitcli::NO_PROMPTS, Some(PREDICT_TIMEOUT));
    parse_merge_tree(&out)
}

/// Reads what `git merge-tree --write-tree --name-only --no-messages -z` printed: exit 0 is a
/// clean merge; exit 1 with a tree id first lists the conflicted paths after it. Anything else
/// (including exit 1 without a tree id, which is how some refusals end) is an error.
pub fn parse_merge_tree(out: &GitOutput) -> Result<Prediction> {
    if out.timed_out {
        return Err(Error::Other("Predicting conflicts took too long, and git was stopped.".into()));
    }
    match out.code {
        Some(0) => return Ok(Prediction::Clean),
        Some(1) => {
            let mut fields = out.stdout.split('\0');
            let tree = fields.next().unwrap_or_default().trim();
            if matches!(tree.len(), 40 | 64) && tree.bytes().all(|b| b.is_ascii_hexdigit()) {
                let mut paths: Vec<String> = Vec::new();
                for f in fields.filter(|f| !f.is_empty()) {
                    if !paths.iter().any(|p| p == f) {
                        paths.push(f.to_owned());
                    }
                }
                return Ok(Prediction::Conflicts(paths));
            }
        }
        Some(129) if out.stderr.contains("usage: git merge-tree") => return Err(too_old(None)),
        _ => {}
    }
    let text = printable(out.stderr.trim());
    Err(Error::Other(if text.is_empty() { "git merge-tree failed".into() } else { text }))
}
