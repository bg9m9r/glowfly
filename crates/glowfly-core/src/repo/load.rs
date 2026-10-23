//! Loading a repository: history, refs, stashes and working-tree status.

use super::graph_cache::GraphCache;
use super::*;
use crate::error::{Error, Result};
use crate::graph;
use git2::{BranchType, Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Loads the repository at (or above) `path`.
pub fn load(path: &Path, opts: &LoadOptions) -> Result<Snapshot> {
    load_cached(path, opts, &GraphCache::default())
}

/// `load`, reusing (and adding to) the ahead/behind and containment answers in `cache`.
pub fn load_cached(path: &Path, opts: &LoadOptions, cache: &GraphCache) -> Result<Snapshot> {
    let _span = tracing::debug_span!("load", path = %path.display(), max = opts.max_commits).entered();
    let loaded_with = opts.clone();
    let max_commits = opts.max_commits.max(1);
    let mut repo = Repository::discover(path).map_err(|e| crate::safe_directory::open_error(path, &e))?;
    if repo.is_bare() {
        return Err(Error::Unsupported("bare repositories are not supported".into()));
    }

    // Taken first, so a change made while loading shows up in the next comparison.
    let refs_key = super::refs_key::key_of(&repo)?;
    // stash_foreach needs &mut, so collect stashes before taking shared borrows.
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        stashes.push(StashInfo { index, message: message.to_owned(), oid: *oid });
        true
    })?;

    let workdir = repo.workdir().ok_or_else(|| Error::Unsupported("repository has no working directory".into()))?.to_path_buf();
    let real_workdir = crate::paths::resolve(&workdir);
    // The spelling it was opened by, when that is another name for the work dir (not a folder
    // inside it): a short 8.3 name or a link the caller may still hold.
    let folder = crate::paths::FolderId::of(&workdir);
    let opened_as = (path != workdir
        && (crate::paths::same_spelling(&crate::paths::resolve(path), &real_workdir)
            || folder.is_some_and(|f| crate::paths::FolderId::of(path) == Some(f))))
    .then(|| path.to_path_buf());
    let git_dir = repo.path().to_path_buf();
    let common = common_dir::CommonDir::of(&repo);
    let name = workdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| workdir.display().to_string());
    let state = repo.state();
    let rebase = crate::rebase::progress_of(&repo);
    let am = crate::patch_ops::progress_of(&repo);

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

    // The checked-out branch is always shown: a saved filter may still hide it after a checkout.
    let mut opts = opts.clone();
    if let Some(b) = &head_branch {
        let full = format!("refs/heads/{b}");
        opts.hidden.retain(|h| *h != full);
    }
    let opts = &opts;

    let head_message = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).and_then(|c| c.message().map(|m| m.trim_end().to_owned()));
    let commit_template = template::commit_template(&repo, &workdir);
    let merge_message = (state == git2::RepositoryState::Merge).then(|| merge_msg::merge_message(&repo)).flatten();
    let gitflow = crate::gitflow::read_config(&repo);
    let lfs_patterns = crate::lfs::root_patterns(&workdir);
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
        if opts.hidden.iter().any(|h| h == full) {
            continue;
        }
        let Ok(commit) = r.peel_to_commit() else { continue };
        let is_head = kind == RefKind::Local && Some(short) == head_branch.as_deref();
        refs.entry(commit.id()).or_default().push(RefLabel { name: short.to_owned(), full: full.to_owned(), kind, is_head });
    }
    for labels in refs.values_mut() {
        labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
    }

    let refs_span = tracing::debug_span!("load.refs").entered();
    let mut locals = Vec::new();
    for b in repo.branches(Some(BranchType::Local))? {
        let (b, _) = b?;
        let name = b.name()?.unwrap_or_default().to_owned();
        let oid = b.get().target();
        let full = format!("refs/heads/{name}");
        let tracking = match (repo.branch_upstream_remote(&full), repo.branch_upstream_merge(&full)) {
            (Ok(r), Ok(m)) => match (r.as_str(), m.as_str()) {
                (Some(r), Some(m)) => Some(Tracking { remote: r.to_owned(), merge: m.strip_prefix("refs/heads/").unwrap_or(m).to_owned() }),
                _ => None,
            },
            _ => None,
        };
        let (upstream, upstream_ref, ahead, behind) = match b.upstream() {
            Ok(u) => {
                let (a, bh) = match (oid, u.get().target()) {
                    (Some(x), Some(y)) => cache.ahead_behind(&repo, x, y),
                    _ => (0, 0),
                };
                (u.name().ok().flatten().map(str::to_owned), u.get().name().map(str::to_owned), a, bh)
            }
            Err(_) => (None, None, 0, 0),
        };
        // Only deleting a branch together with its remote branch asks, and the walk can be long.
        let remote_upstream = upstream_ref.as_deref().is_some_and(|r| r.starts_with("refs/remotes/"));
        let merged = match (oid, head_oid) {
            (Some(tip), Some(head)) => tip == head || (remote_upstream && cache.contains(&repo, head, tip)),
            _ => false,
        };
        locals.push(BranchInfo { is_head: b.is_head(), name, oid, upstream, ahead, behind, tracking, upstream_ref, merged });
    }

    let mut remotes: Vec<(String, Vec<(String, Oid)>)> = repo.remotes()?.iter().flatten().map(|r| (r.to_owned(), Vec::new())).collect();
    for b in repo.branches(Some(BranchType::Remote))? {
        let (b, _) = b?;
        let (Some(name), Some(oid)) = (b.name()?.map(str::to_owned), b.get().target()) else {
            continue;
        };
        if name.ends_with("/HEAD") {
            continue;
        }
        // Remote names may contain '/', so pick the longest matching remote prefix.
        if let Some(slot) = remotes.iter_mut().filter(|(r, _)| name.starts_with(&format!("{r}/"))).max_by_key(|(r, _)| r.len()) {
            let short = name[slot.0.len() + 1..].to_owned();
            slot.1.push((short, oid));
        }
    }

    let names: Vec<&str> = remotes.iter().map(|(r, _)| r.as_str()).collect();
    let remote_info = super::remotes::remote_info(&repo, &names);
    let remote_heads = super::remotes::remote_heads(&repo, &names);
    let remote_urls = remotes.iter().filter_map(|(r, _)| Some((r.clone(), repo.find_remote(r).ok()?.url()?.to_owned()))).collect();

    let (mut tags, mut other_tags, mut lightweight_tags) = (Vec::new(), Vec::new(), HashSet::new());
    for t in repo.tag_names(None)?.iter().flatten() {
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

    // Git's own status read can run the repository's fsmonitor hook or filters: only once the
    // user trusts it (and nothing was added since), or when nothing in it would run.
    drop(refs_span);
    let trust_scan = crate::trust::scan(&repo);
    let trust_print = trust_scan.print();
    let git_status = trust_scan.is_empty() || (opts.trusted && opts.trusted_print.as_ref().is_none_or(|p| p.covers(&trust_print)));
    // Found once per load, before the status read that marks its rows with them.
    let collisions = tracing::debug_span!("load.collisions").in_scope(|| repo.index().map(|mut i| super::collide::collisions(&mut i)))?;
    let started = std::time::Instant::now();
    // Read before the status, so a change made during the read shows up in the next comparison.
    let index_memo = repo.index().ok().and_then(|i| super::IndexMemo::of(&i)).map(std::sync::Arc::new);
    let (unstaged, staged) =
        tracing::debug_span!("load.status").in_scope(|| super::status::load_status_with(&repo, git_status, &collisions))?;
    let slow_status = super::status_speed::slow(started.elapsed(), &workdir);
    let changes = ChangeCounts::of(&staged, &unstaged);
    let folding = super::collide::folding(repo.path());
    let ignorecase = super::collide::ignorecase(&repo);
    let submodules = submodules::list(&repo);
    let mut worktrees = worktrees::list(&repo);
    if opts.worktree_wips {
        super::worktree_counts::count_in_load(&mut worktrees, &super::worktree_counts::LoadLimits::DEFAULT);
    }
    let shallow_roots = super::shallow::roots(&repo);
    let fetched_at = super::fetch_time::fetched_at(&git_dir, &common.dir);

    let mut starts: Vec<Oid> = head_oid.into_iter().collect();
    let tips = visible_tips(&repo, opts, head_branch.as_deref(), &locals);
    match &tips {
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
    // Labels follow the same visibility as the history.
    if let Some(list) = &tips {
        for labels in refs.values_mut() {
            labels.retain(|l| l.kind == RefKind::Head || l.is_head || list.contains(&l.full));
        }
        refs.retain(|_, v| !v.is_empty());
    }
    let (commits, truncated) = tracing::debug_span!("load.walk").in_scope(|| super::walk::newest_commits(&repo, &starts, max_commits))?;
    let row_of = commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();

    let mut snap = Snapshot {
        workdir,
        real_workdir,
        opened_as,
        folder,
        git_dir,
        common,
        name,
        remote_urls,
        head_branch,
        head_oid,
        head_message,
        commit_template,
        merge_message,
        gitflow,
        lfs_patterns,
        state,
        rebase,
        am,
        commits: commits.into(),
        row_of,
        truncated,
        starts,
        refs,
        locals,
        remotes,
        remote_info,
        remote_heads,
        tags,
        other_tags,
        lightweight_tags,
        stashes,
        unstaged,
        staged,
        changes,
        slow_status,
        index_memo,
        folding,
        ignorecase,
        collisions,
        submodules,
        worktrees,
        trust_scan,
        trust_print,
        shallow_roots,
        fetched_at,
        graph: graph::Layout::default(),
        stacks: Default::default(),
        refs_key,
        loaded_with,
    };
    tracing::debug_span!("load.layout").in_scope(|| snap.lay_out_graph());
    snap.stacks = crate::stack::read(&repo, &snap);
    Ok(snap)
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

/// Messages of commits reachable from `source` but not from `target` (newest first, capped).
pub fn messages_between(workdir: &Path, source: &str, target: &str) -> Result<Vec<String>> {
    let repo = Repository::open(workdir)?;
    let mut walk = repo.revwalk()?;
    walk.push(repo.revparse_single(source)?.peel_to_commit()?.id())?;
    if let Ok(t) = repo.revparse_single(target).and_then(|o| o.peel_to_commit()) {
        walk.hide(t.id())?;
    }
    let mut out = Vec::new();
    for oid in walk.take(500) {
        if let Some(m) = repo.find_commit(oid?)?.message() {
            out.push(m.to_owned());
        }
    }
    Ok(out)
}
