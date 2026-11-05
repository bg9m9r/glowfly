//! A load's history walk: which tips it starts from (a filtered graph's, or every ref not
//! hidden), the ref labels it keeps, and the walk itself, taking the commits an earlier load read
//! (`walk_reuse`) and then marking the merged branches. Declared from `load.rs`, so its paths
//! into `repo` are spelled in full.

use crate::error::Result;
use crate::repo::graph_cache::GraphCache;
use crate::repo::{BranchInfo, CommitInfo, Frontier, LoadOptions, Prior, RefKind, RefLabel};
use git2::{Oid, Repository};
use std::collections::HashMap;

/// What the walk found: where it started, the commits newest first with each one's row, and
/// where a walk cut short at the limit stopped.
pub(super) struct History {
    pub starts: Vec<Oid>,
    pub commits: Vec<CommitInfo>,
    pub row_of: HashMap<Oid, usize>,
    pub truncated: bool,
    pub frontier: Option<Frontier>,
}

/// The checked-out commit and branch.
pub(super) type Head<'a> = (Option<Oid>, Option<&'a str>);

/// Walks at most `max_commits` of the history from HEAD and the tips `opts` shows, keeping only
/// the labels (`refs`) of the refs walked, reading again only the commits `prior` lacks, then
/// marks which of `locals` are merged into HEAD.
pub(super) fn history(
    repo: &Repository,
    (opts, max_commits): (&LoadOptions, usize),
    (cache, prior): (&GraphCache, Option<&Prior>),
    (head_oid, head_branch): Head,
    shallow_roots: &[Oid],
    refs: &mut HashMap<Oid, Vec<RefLabel>>,
    locals: &mut [BranchInfo],
) -> Result<History> {
    let tips = visible_tips(repo, opts, head_branch, locals);
    let starts = walk_starts(repo, opts, head_oid, tips.as_deref())?;
    // Labels follow the same visibility as the history.
    keep_visible_labels(refs, tips.as_deref());
    let reuse = prior.and_then(|p| crate::repo::walk_reuse::CommitCache::new(p, repo, shallow_roots));
    let walked =
        tracing::debug_span!("load.walk").in_scope(|| crate::repo::walk::newest_commits(repo, &starts, max_commits, reuse.as_ref()))?;
    drop(reuse);
    let (commits, truncated) = (walked.commits, walked.truncated);
    let row_of = commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();
    crate::repo::branch_merged::mark(repo, cache, head_oid, &commits, &row_of, locals);
    Ok(History { starts, commits, row_of, truncated, frontier: walked.frontier })
}

/// Keeps only the labels of refs a filtered graph walks (`tips`), and HEAD's.
fn keep_visible_labels(refs: &mut HashMap<Oid, Vec<RefLabel>>, tips: Option<&[String]>) {
    if let Some(list) = tips {
        for labels in refs.values_mut() {
            labels.retain(|l| l.kind == RefKind::Head || l.is_head || list.contains(&l.full));
        }
        refs.retain(|_, v| !v.is_empty());
    }
}

/// Where the history walk starts: HEAD, then the commits of `tips` (a filtered graph's) or of
/// every branch, remote branch and tag `opts` doesn't hide.
fn walk_starts(repo: &Repository, opts: &LoadOptions, head_oid: Option<Oid>, tips: Option<&[String]>) -> Result<Vec<Oid>> {
    let mut starts: Vec<Oid> = head_oid.into_iter().collect();
    match tips {
        Some(list) => {
            for full in list {
                if let Ok(c) = repo.revparse_single(full).and_then(|o| o.peel_to_commit()) {
                    starts.push(c.id());
                }
            }
        }
        None => {
            for r in repo.references()?.flatten() {
                let Some(full) = r.name() else { continue };
                let wanted = full.starts_with("refs/heads/") || full.starts_with("refs/remotes/") || full.starts_with("refs/tags/");
                if !wanted || opts.hidden.iter().any(|h| h == full) {
                    continue;
                }
                if let Ok(c) = r.peel_to_commit() {
                    starts.push(c.id());
                }
            }
        }
    }
    Ok(starts)
}

/// Ref names to walk when the graph is filtered; None means everything.
fn visible_tips(repo: &Repository, opts: &LoadOptions, head_branch: Option<&str>, locals: &[BranchInfo]) -> Option<Vec<String>> {
    if !opts.solo.is_empty() {
        return Some(opts.solo.clone());
    }
    if !opts.smart {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: String| {
        // Hidden refs are not walked as smart tips; HEAD is pushed separately.
        if opts.hidden.contains(&name) {
            return;
        }
        if !out.contains(&name) && repo.revparse_single(&name).is_ok() {
            out.push(name);
        }
    };
    if let Some(b) = head_branch {
        add(format!("refs/heads/{b}"));
    }
    // Upstreams of the current and default branches.
    for b in locals {
        let default = matches!(b.name.as_str(), "main" | "master" | "develop" | "trunk");
        if b.is_head || default {
            add(format!("refs/heads/{}", b.name));
            if let Some(u) = &b.upstream {
                add(format!("refs/remotes/{u}"));
            }
        }
    }
    // Each remote's default branch (refs/remotes/<remote>/HEAD).
    if let Ok(remotes) = repo.remotes() {
        for r in remotes.iter().flatten() {
            if let Ok(target) = repo.find_reference(&format!("refs/remotes/{r}/HEAD")).and_then(|h| h.resolve())
                && let Some(name) = target.name()
            {
                add(name.to_owned());
            }
        }
    }
    for b in &opts.extra_tips {
        add(format!("refs/heads/{b}"));
        if let Ok(remotes) = repo.remotes() {
            for r in remotes.iter().flatten() {
                add(format!("refs/remotes/{r}/{b}"));
            }
        }
    }
    Some(out)
}
