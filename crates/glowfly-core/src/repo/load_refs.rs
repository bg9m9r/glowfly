//! A load's reads of HEAD and the refs: the checked-out branch and, from one pass over the
//! references (`RefPass`), the labels the graph puts on commits, the history's starting points,
//! the local branches, the remote branches and the tags; and the remotes' settings. The pass
//! reads each ref once, and the targets are peeled from object headers (`ref_peel`) without
//! reading commits. The upstreams are filled in by `upstream_config`. Declared from `load.rs`, so
//! its paths into `repo` are spelled in full.

use super::ref_peel::{Peeled, Peeler};
use crate::error::Result;
use crate::repo::{BranchInfo, LoadOptions, RefKind, RefLabel, RemoteInfo, is_safe_ref_name, safe_names};
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};

/// HEAD's commit, its branch and whether it is detached. An unborn branch has its name and no
/// commit; a branch whose name git could read as an option shows as a detached HEAD, so no
/// command is built from its name.
pub(super) fn head_of(repo: &Repository) -> (Option<Oid>, Option<String>, bool) {
    let (head_oid, head_branch, detached) = match repo.head() {
        Ok(h) => {
            let branch = if h.is_branch() { h.shorthand().map(str::to_owned) } else { None };
            (h.target(), branch, !h.is_branch())
        }
        Err(_) => {
            // Unborn branch: HEAD is symbolic but points nowhere yet.
            let branch =
                repo.find_reference("HEAD").ok().and_then(|r| r.symbolic_target().map(|s| s.trim_start_matches("refs/heads/").to_owned()));
            (None, branch, false)
        }
    };
    match head_branch {
        Some(b) if !is_safe_ref_name(&b) => (head_oid, None, true),
        b => (head_oid, b, detached),
    }
}

/// Which namespace a ref is in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Local,
    Remote,
    Tag,
}

/// A branch, remote branch or tag ref with a UTF-8 name.
struct Entry {
    full: String,
    kind: Kind,
    /// The target of a direct ref; None for a symbolic one.
    direct: Option<Oid>,
    /// `direct`, or for a symbolic ref the target it resolves to (None when dangling).
    resolved: Option<Oid>,
}

impl Entry {
    /// The name after `refs/heads/`, `refs/remotes/` or `refs/tags/`.
    fn short(&self) -> &str {
        let prefix = match self.kind {
            Kind::Local => "refs/heads/",
            Kind::Remote => "refs/remotes/",
            Kind::Tag => "refs/tags/",
        };
        &self.full[prefix.len()..]
    }
}

/// HEAD as `head_of` reads it: its commit, its branch and whether it is detached.
pub(super) type Head<'a> = (Option<Oid>, Option<&'a str>, bool);

/// One pass over the repository's references, in libgit2's order, keeping what the load reads of
/// the branches, remote branches and tags (nothing else is kept).
pub(super) struct RefPass {
    entries: Vec<Entry>,
}

impl RefPass {
    /// Iterates the references once.
    pub(super) fn read(repo: &Repository) -> Result<RefPass> {
        let mut entries = Vec::new();
        for r in repo.references()?.flatten() {
            let Some(full) = r.name() else { continue };
            let kind = if full.starts_with("refs/heads/") {
                Kind::Local
            } else if full.starts_with("refs/remotes/") {
                Kind::Remote
            } else if full.starts_with("refs/tags/") {
                Kind::Tag
            } else {
                continue;
            };
            let direct = r.target();
            let resolved = direct.or_else(|| r.resolve().ok().and_then(|x| x.target()));
            entries.push(Entry { full: full.to_owned(), kind, direct, resolved });
        }
        Ok(RefPass { entries })
    }

    /// The ref labels by commit (a detached HEAD's own, then branches, remote branches and tags
    /// not hidden by `opts`), each commit's sorted HEAD first, then by kind and name.
    pub(super) fn labels(
        &self,
        peeler: &mut Peeler,
        opts: &LoadOptions,
        (head_oid, head_branch, detached): Head,
    ) -> HashMap<Oid, Vec<RefLabel>> {
        let mut refs: HashMap<Oid, Vec<RefLabel>> = HashMap::new();
        if detached && let Some(o) = head_oid {
            refs.entry(o).or_default().push(RefLabel { name: "HEAD".into(), full: String::new(), kind: RefKind::Head, is_head: true });
        }
        for e in &self.entries {
            let short = e.short();
            // A remote's own HEAD gets no label, nor does a name git could read as an option.
            let skipped = (e.kind == Kind::Remote && short.ends_with("/HEAD")) || !is_safe_ref_name(short);
            if skipped || opts.hidden.contains(&e.full) {
                continue;
            }
            let Some(commit) = e.resolved.and_then(|t| peeler.peel(t).commit()) else { continue };
            let kind = match e.kind {
                Kind::Local => RefKind::Local,
                Kind::Remote => RefKind::Remote,
                Kind::Tag => RefKind::Tag,
            };
            refs.entry(commit).or_default().push(RefLabel {
                name: short.to_owned(),
                full: e.full.clone(),
                kind,
                is_head: kind == RefKind::Local && Some(short) == head_branch,
            });
        }
        for labels in refs.values_mut() {
            labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
        }
        refs
    }

    /// The commits an unfiltered history walk starts from after HEAD: every branch, remote
    /// branch and tag `opts` doesn't hide, remote HEADs and unsafe names included.
    pub(super) fn starts(&self, peeler: &mut Peeler, opts: &LoadOptions) -> Vec<Oid> {
        let shown = self.entries.iter().filter(|e| !opts.hidden.contains(&e.full));
        shown.filter_map(|e| e.resolved.and_then(|t| peeler.peel(t).commit())).collect()
    }

    /// Every local and remote branch ref's direct target (None for a symbolic ref), by its full
    /// name, whatever the name: the upstreams are looked up here.
    pub(super) fn tips(&self) -> HashMap<&str, Option<Oid>> {
        self.entries.iter().filter(|e| e.kind != Kind::Tag).map(|e| (e.full.as_str(), e.direct)).collect()
    }

    /// The local branches, with no upstream yet (`upstream_config::fill_upstreams` and, once the
    /// remotes are read, `push_target` set them). `merged` is worked out after the walk, from the
    /// loaded rows (`branch_merged`).
    pub(super) fn local_branches(&self, head_branch: Option<&str>) -> Vec<BranchInfo> {
        let locals = self.entries.iter().filter(|e| e.kind == Kind::Local && is_safe_ref_name(e.short()));
        locals
            .map(|e| BranchInfo {
                is_head: Some(e.short()) == head_branch,
                name: e.short().to_owned(),
                oid: e.direct,
                upstream: None,
                ahead: 0,
                behind: 0,
                tracking: None,
                upstream_ref: None,
                merged: false,
                push: None,
            })
            .collect()
    }

    /// The repository's remotes and their branches, as `Remotes`.
    pub(super) fn remote_branches(&self, repo: &Repository) -> Result<Remotes> {
        let mut remotes: Remotes =
            repo.remotes()?.iter().flatten().filter(|r| safe_names::remote_ok(r)).map(|r| (r.to_owned(), Vec::new())).collect();
        for e in self.entries.iter().filter(|e| e.kind == Kind::Remote) {
            let (name, Some(oid)) = (e.short(), e.direct) else { continue };
            if name.ends_with("/HEAD") || !is_safe_ref_name(name) {
                continue;
            }
            // Remote names may contain '/', so pick the longest matching remote prefix.
            let under = |r: &str| name.strip_prefix(r).is_some_and(|rest| rest.starts_with('/'));
            if let Some(slot) = remotes.iter_mut().filter(|(r, _)| under(r)).max_by_key(|(r, _)| r.len()) {
                slot.1.push((name[slot.0.len() + 1..].to_owned(), oid));
            }
        }
        Ok(remotes)
    }

    /// The repository's tags, as `Tags`.
    pub(super) fn tags(&self, peeler: &mut Peeler) -> Tags {
        let (mut tags, mut other_tags, mut lightweight_tags) = (Vec::new(), Vec::new(), HashSet::new());
        for e in self.entries.iter().filter(|e| e.kind == Kind::Tag && is_safe_ref_name(e.short())) {
            // A dangling symbolic tag names nothing.
            let Some(target) = e.resolved else { continue };
            let name = e.short().to_owned();
            match peeler.peel(target) {
                // A ref that names the commit itself has no tag object: a lightweight tag.
                Peeled::Commit(c) => {
                    lightweight_tags.insert(name.clone());
                    tags.push((name, c));
                }
                Peeled::Tag(c) => tags.push((name, c)),
                Peeled::Other => other_tags.push(name),
            }
        }
        tags.reverse();
        (tags, other_tags, lightweight_tags)
    }
}

/// Each remote with its branches (short name and tip).
pub(super) type Remotes = Vec<(String, Vec<(String, Oid)>)>;

/// The tags: those naming a commit with their commits (in the reverse of git's listing), the
/// others, and the names of the lightweight ones.
pub(super) type Tags = (Vec<(String, Oid)>, Vec<String>, HashSet<String>);

/// What a load reads of the remotes' settings, beside their branches.
pub(super) struct RemoteReads {
    pub remote_info: Vec<RemoteInfo>,
    pub remote_heads: Vec<(String, String)>,
    /// Whether pulling the checked-out branch rebases.
    pub pull_rebase: bool,
    /// Each remote's fetch URL.
    pub remote_urls: Vec<(String, String)>,
}

/// Reads the settings of `remotes` (and the pull one of `head_branch`). Which of them
/// FETCH_HEAD names is read by the load later, straight after FETCH_HEAD's time, so the two match.
pub(super) fn remote_reads(repo: &Repository, remotes: &Remotes, head_branch: Option<&str>) -> RemoteReads {
    let names: Vec<&str> = remotes.iter().map(|(r, _)| r.as_str()).collect();
    let remote_info = crate::repo::remotes::remote_info(repo, &names);
    let remote_heads = crate::repo::remotes::remote_heads(repo, &names);
    let pull_rebase = crate::repo::push_config::pull_rebases(repo, head_branch);
    let remote_urls: Vec<(String, String)> =
        remotes.iter().filter_map(|(r, _)| Some((r.clone(), repo.find_remote(r).ok()?.url()?.to_owned()))).collect();
    RemoteReads { remote_info, remote_heads, pull_rebase, remote_urls }
}

#[cfg(test)]
#[path = "load_refs_parity_tests.rs"]
mod parity_tests;
#[cfg(test)]
#[path = "load_refs_tests.rs"]
mod tests;
