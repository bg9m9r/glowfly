//! A load's reads of HEAD and the refs: the checked-out branch, the labels the graph puts on
//! commits, the local branches with their upstreams, the remote branches, the tags and the
//! remotes' settings. Declared from `load.rs`, so its paths into `repo` are spelled in full.

use crate::error::Result;
use crate::repo::graph_cache::GraphCache;
use crate::repo::{BranchInfo, LoadOptions, PushRemotes, RefKind, RefLabel, RemoteInfo, Tracking, is_safe_ref_name, safe_names};
use git2::{BranchType, Oid, Repository};
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

/// The ref labels by commit (a detached HEAD's own, then branches, remote branches and tags not
/// hidden by `opts`), each commit's sorted HEAD first, then by kind and name.
pub(super) fn ref_labels(
    repo: &Repository,
    opts: &LoadOptions,
    (head_oid, head_branch, detached): (Option<Oid>, Option<&str>, bool),
) -> Result<HashMap<Oid, Vec<RefLabel>>> {
    let mut refs: HashMap<Oid, Vec<RefLabel>> = HashMap::new();
    if detached && let Some(o) = head_oid {
        refs.entry(o).or_default().push(RefLabel { name: "HEAD".into(), full: String::new(), kind: RefKind::Head, is_head: true });
    }
    for r in repo.references()?.flatten() {
        let Some(full) = r.name() else { continue };
        let (kind, short) = if let Some(s) = full.strip_prefix("refs/heads/") {
            (RefKind::Local, s)
        } else if let Some(s) = full.strip_prefix("refs/remotes/") {
            if s.ends_with("/HEAD") {
                continue;
            }
            (RefKind::Remote, s)
        } else if let Some(s) = full.strip_prefix("refs/tags/") {
            (RefKind::Tag, s)
        } else {
            continue;
        };
        // A name git could read as an option is left out.
        if opts.hidden.iter().any(|h| h == full) || !is_safe_ref_name(short) {
            continue;
        }
        let Ok(commit) = r.peel_to_commit() else { continue };
        refs.entry(commit.id()).or_default().push(RefLabel {
            name: short.to_owned(),
            full: full.to_owned(),
            kind,
            is_head: kind == RefKind::Local && Some(short) == head_branch,
        });
    }
    for labels in refs.values_mut() {
        labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
    }
    Ok(refs)
}

/// The local branches with their upstreams and ahead/behind counts (from `cache`).
pub(super) fn local_branches(repo: &Repository, cache: &GraphCache) -> Result<Vec<BranchInfo>> {
    let mut locals = Vec::new();
    for b in repo.branches(Some(BranchType::Local))? {
        let (b, _) = b?;
        let name = b.name()?.unwrap_or_default().to_owned();
        if !is_safe_ref_name(&name) {
            continue;
        }
        let oid = b.get().target();
        let full = format!("refs/heads/{name}");
        let tracking = match (repo.branch_upstream_remote(&full), repo.branch_upstream_merge(&full)) {
            (Ok(r), Ok(m)) => match (r.as_str(), m.as_str()) {
                (Some(r), Some(m)) => Some(Tracking { remote: r.to_owned(), merge: m.strip_prefix("refs/heads/").unwrap_or(m).to_owned() }),
                _ => None,
            },
            _ => None,
        };
        // An upstream whose remote or branch name git could misread is dropped as a whole.
        let tracking = tracking.filter(|t| safe_names::upstream_remote_ok(&t.remote) && is_safe_ref_name(&t.merge));
        let (upstream, upstream_ref, ahead, behind) = match b.upstream() {
            Ok(u) if tracking.is_some() => {
                let (a, bh) = match (oid, u.get().target()) {
                    (Some(x), Some(y)) => cache.ahead_behind(repo, x, y),
                    _ => (0, 0),
                };
                (u.name().ok().flatten().map(str::to_owned), u.get().name().map(str::to_owned), a, bh)
            }
            _ => (None, None, 0, 0),
        };
        // `merged` is worked out after the walk, from the loaded rows (`branch_merged`); `push`
        // once the remotes are read (`push_target`).
        locals.push(BranchInfo {
            is_head: b.is_head(),
            name,
            oid,
            upstream,
            ahead,
            behind,
            tracking,
            upstream_ref,
            merged: false,
            push: None,
        });
    }
    Ok(locals)
}

/// Each remote with its branches (short name and tip).
pub(super) type Remotes = Vec<(String, Vec<(String, Oid)>)>;

/// The repository's remotes and their branches, as `Remotes`.
pub(super) fn remote_branches(repo: &Repository) -> Result<Remotes> {
    let mut remotes: Remotes =
        repo.remotes()?.iter().flatten().filter(|r| safe_names::remote_ok(r)).map(|r| (r.to_owned(), Vec::new())).collect();
    for b in repo.branches(Some(BranchType::Remote))? {
        let (b, _) = b?;
        let (Some(name), Some(oid)) = (b.name()?.map(str::to_owned), b.get().target()) else {
            continue;
        };
        if name.ends_with("/HEAD") || !is_safe_ref_name(&name) {
            continue;
        }
        // Remote names may contain '/', so pick the longest matching remote prefix.
        if let Some(slot) = remotes.iter_mut().filter(|(r, _)| name.starts_with(&format!("{r}/"))).max_by_key(|(r, _)| r.len()) {
            let short = name[slot.0.len() + 1..].to_owned();
            slot.1.push((short, oid));
        }
    }
    Ok(remotes)
}

/// The tags: those naming a commit with their commits (in the reverse of git's listing), the
/// others, and the names of the lightweight ones.
pub(super) type Tags = (Vec<(String, Oid)>, Vec<String>, HashSet<String>);

/// The repository's tags, as `Tags`.
pub(super) fn tags(repo: &Repository) -> Result<Tags> {
    let (mut tags, mut other_tags, mut lightweight_tags) = (Vec::new(), Vec::new(), HashSet::new());
    for t in repo.tag_names(None)?.iter().flatten().filter(|t| is_safe_ref_name(t)) {
        // A ref that names the commit itself has no tag object: a lightweight tag.
        let found = repo.revparse_single(&format!("refs/tags/{t}")).and_then(|o| {
            let light = o.kind() == Some(git2::ObjectType::Commit);
            o.peel_to_commit().map(|c| (c, light))
        });
        match found {
            Ok((c, light)) => {
                if light {
                    lightweight_tags.insert(t.to_owned());
                }
                tags.push((t.to_owned(), c.id()));
            }
            Err(_) => other_tags.push(t.to_owned()),
        }
    }
    tags.reverse();
    Ok((tags, other_tags, lightweight_tags))
}

/// What a load reads of the remotes' settings, beside their branches.
pub(super) struct RemoteReads {
    pub remote_info: Vec<RemoteInfo>,
    pub remote_heads: Vec<(String, String)>,
    pub push_remotes: PushRemotes,
    /// Whether pulling the checked-out branch rebases.
    pub pull_rebase: bool,
    /// Each remote's fetch URL.
    pub remote_urls: Vec<(String, String)>,
}

/// Reads the settings of `remotes` (and the push and pull ones of `head_branch`). Which of them
/// FETCH_HEAD names is read by the load later, straight after FETCH_HEAD's time, so the two match.
pub(super) fn remote_reads(repo: &Repository, remotes: &Remotes, head_branch: Option<&str>) -> RemoteReads {
    let names: Vec<&str> = remotes.iter().map(|(r, _)| r.as_str()).collect();
    let remote_info = crate::repo::remotes::remote_info(repo, &names);
    let remote_heads = crate::repo::remotes::remote_heads(repo, &names);
    let push_remotes = crate::repo::push_config::push_remotes(repo);
    let pull_rebase = crate::repo::push_config::pull_rebases(repo, head_branch);
    let remote_urls: Vec<(String, String)> =
        remotes.iter().filter_map(|(r, _)| Some((r.clone(), repo.find_remote(r).ok()?.url()?.to_owned()))).collect();
    RemoteReads { remote_info, remote_heads, push_remotes, pull_rebase, remote_urls }
}

#[cfg(test)]
#[path = "load_refs_tests.rs"]
mod tests;
