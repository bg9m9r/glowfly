//! Loading a repository: history, refs, stashes and working-tree status. The refs are read in
//! `load_refs.rs`, the history in `load_walk.rs`.

use super::graph_cache::GraphCache;
use super::*;
use crate::branch_ref::BranchRef;
use crate::commit_text;
use crate::error::{Error, Result};
use crate::graph;
use git2::Repository;
use std::path::{Path, PathBuf};

#[path = "head_ahead.rs"]
mod head_ahead;
#[path = "load_refs.rs"]
mod load_refs;
#[path = "load_walk.rs"]
mod load_walk;
#[path = "merge_dest.rs"]
mod merge_dest;

/// Loads the repository at (or above) `path`.
pub fn load(path: &Path, opts: &LoadOptions) -> Result<Snapshot> {
    load_cached(path, opts, &GraphCache::default())
}

/// `load`, reusing (and adding to) the ahead/behind and containment answers in `cache`.
pub fn load_cached(path: &Path, opts: &LoadOptions, cache: &GraphCache) -> Result<Snapshot> {
    load_reusing(path, opts, cache, None)
}

/// `load_cached`, taking the commits of `prior` (the snapshot on screen's history) instead of
/// reading them again (`walk_reuse`); the result is the same as without it.
pub fn load_reusing(path: &Path, opts: &LoadOptions, cache: &GraphCache, prior: Option<&Prior>) -> Result<Snapshot> {
    let _span = tracing::debug_span!("load", path = %path.display(), max = opts.max_commits).entered();
    let loaded_with = opts.clone();
    let max_commits = opts.max_commits.max(1);
    let repo = crate::safe_directory::discover(path)?;
    if repo.is_bare() {
        return Err(Error::Unsupported("bare repositories are not supported".into()));
    }

    // Taken first, so a change made while loading shows up in the next comparison.
    let refs_key = super::refs_key::key_of(&repo)?;
    let stashes = crate::stash_ops::list_in(&repo)?;
    let Place { workdir, real_workdir, opened_as, folder, git_dir, common, name } = place_of(path, &repo)?;
    let state = op_state::state(&repo);
    let rebase = crate::rebase::progress_of(&repo);
    let am = crate::patch_ops::progress_of(&repo);

    let (head_oid, head_branch, detached) = load_refs::head_of(&repo);
    let opts = &shown_opts(opts, head_branch.as_deref());
    let BoxReads { head_message, commit_template, commit_encoding, merge_message, gitflow, lfs_patterns } =
        box_reads(&repo, &workdir, state);
    let mut refs = load_refs::ref_labels(&repo, opts, (head_oid, head_branch.as_deref(), detached))?;

    let refs_span = tracing::debug_span!("load.refs").entered();
    let mut locals = load_refs::local_branches(&repo, cache)?;
    let remotes = load_refs::remote_branches(&repo)?;
    let load_refs::RemoteReads { remote_info, remote_heads, push_remotes, pull_rebase, remote_urls } =
        load_refs::remote_reads(&repo, &remotes, head_branch.as_deref());
    let merge_names = merge_dest::merge_names(&repo, head_branch.as_deref(), &branch_refs(&locals, &remotes));
    let remote_names: Vec<&str> = remotes.iter().map(|(r, _)| r.as_str()).collect();
    super::push_target::fill(&repo, cache, &mut locals, &push_remotes, &remote_names);
    let (tags, other_tags, lightweight_tags) = load_refs::tags(&repo)?;
    drop(refs_span);

    let StatusRead { trust_scan, trust_print, collisions, index_memo, unstaged, staged, slow_status, changes } =
        read_status(&repo, opts, cache, &workdir)?;
    let folding = super::collide::folding(repo.path());
    let ignorecase = super::collide::ignorecase(&repo);
    let submodules = submodules::list(&repo, Some(&unstaged), Some(cache.submodules()));
    let submodules_unread = crate::gitmodules::unread_anywhere(&repo);
    let mut worktrees = worktrees::list(&repo);
    if opts.worktree_wips {
        super::worktree_counts::count_in_load(&mut worktrees, repo.commondir(), &super::worktree_counts::LoadLimits::DEFAULT);
    }
    let shallow_roots = super::shallow::roots(&repo);
    let fetched_at = super::fetch_time::fetched_at(&git_dir, &common.dir);
    let fetched_remotes = super::fetch_time::fetched_remotes(&git_dir, &common.dir, &remote_info, &remote_urls);

    let head = (head_oid, head_branch.as_deref());
    let load_walk::History { starts, commits, row_of, truncated, frontier } =
        load_walk::history(&repo, (opts, max_commits), (cache, prior), head, &shallow_roots, &mut refs, &mut locals)?;
    let head_ahead = head_ahead::first_parent(&repo, head_oid, &locals);

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
        commit_encoding,
        merge_message,
        gitflow,
        lfs_patterns,
        state,
        rebase,
        am,
        commits: commits.into(),
        row_of,
        truncated,
        frontier: frontier.map(std::sync::Arc::new),
        starts,
        refs,
        locals,
        head_ahead,
        remotes,
        remote_info,
        remote_heads,
        push_remotes,
        pull_rebase,
        merge_names,
        tags,
        other_tags,
        lightweight_tags,
        stashes,
        unstaged,
        staged,
        changes,
        status_stamp: super::next_status_stamp(),
        slow_status,
        index_memo,
        folding,
        ignorecase,
        collisions,
        submodules_unread,
        submodules,
        worktrees,
        trust_scan,
        trust_print,
        shallow_roots,
        fetched_at,
        fetched_remotes,
        graph: graph::Layout::default(),
        stacks: Default::default(),
        refs_key,
        loaded_with,
    };
    tracing::debug_span!("load.layout").in_scope(|| snap.lay_out_graph());
    snap.stacks = crate::stack::read(&repo, &snap);
    Ok(snap)
}

/// Every local and remote-tracking branch, as the menus name them.
fn branch_refs(locals: &[BranchInfo], remotes: &load_refs::Remotes) -> Vec<BranchRef> {
    let remote = remotes.iter().flat_map(|(r, bs)| bs.iter().map(move |(b, _)| BranchRef::Remote(format!("{r}/{b}"))));
    locals.iter().map(|b| BranchRef::Local(b.name.clone())).chain(remote).collect()
}

/// Where the repository is: its work dir (as found and resolved), the spelling it was opened
/// by, its folder's identity, its git and common dirs, and the name it is shown by.
struct Place {
    workdir: PathBuf,
    real_workdir: PathBuf,
    opened_as: Option<PathBuf>,
    folder: Option<crate::paths::FolderId>,
    git_dir: PathBuf,
    common: CommonDir,
    name: String,
}

/// The `Place` of `repo`, opened by `path`; an error for a repository without a work dir.
fn place_of(path: &Path, repo: &Repository) -> Result<Place> {
    let workdir = repo.workdir().ok_or_else(|| Error::Unsupported("repository has no working directory".into()))?.to_path_buf();
    let real_workdir = crate::paths::resolve(&workdir);
    let folder = crate::paths::FolderId::of(&workdir);
    let opened_as = opened_as(path, &workdir, &real_workdir, folder);
    let git_dir = repo.path().to_path_buf();
    let common = common_dir::CommonDir::of(repo);
    let name = workdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| workdir.display().to_string());
    Ok(Place { workdir, real_workdir, opened_as, folder, git_dir, common, name })
}

/// The spelling the repository was opened by (`path`), when that is another name for the work
/// dir (not a folder inside it): a short 8.3 name or a link the caller may still hold.
fn opened_as(path: &Path, workdir: &Path, real_workdir: &Path, folder: Option<crate::paths::FolderId>) -> Option<PathBuf> {
    (path != workdir
        && (crate::paths::same_spelling(&crate::paths::resolve(path), real_workdir)
            || folder.is_some_and(|f| crate::paths::FolderId::of(path) == Some(f))))
    .then(|| path.to_path_buf())
}

/// `opts` with the checked-out branch never hidden: a saved filter may still hide it after a
/// checkout.
fn shown_opts(opts: &LoadOptions, head_branch: Option<&str>) -> LoadOptions {
    let mut opts = opts.clone();
    if let Some(b) = head_branch {
        let full = format!("refs/heads/{b}");
        opts.hidden.retain(|h| *h != full);
    }
    opts
}

/// What the commit box and the operation banners start from: HEAD's message, the commit
/// template and encoding, a stopped merge's message, the git-flow settings and the LFS patterns.
struct BoxReads {
    head_message: Option<String>,
    commit_template: Option<CommitTemplate>,
    commit_encoding: crate::commit_encoding::MessageEncoding,
    merge_message: Option<String>,
    gitflow: crate::gitflow::GitFlowConfig,
    lfs_patterns: Vec<crate::lfs::LfsPattern>,
}

/// Reads the `BoxReads` of `repo` (work dir `workdir`) in operation `state`.
fn box_reads(repo: &Repository, workdir: &Path, state: git2::RepositoryState) -> BoxReads {
    let head_message = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).map(|c| commit_text::message(&c).trim_end().to_owned());
    let commit_template = template::commit_template(repo, workdir);
    let commit_encoding = crate::commit_encoding::MessageEncoding::of(repo);
    let merge_message = op_state::has_merge_msg(state).then(|| merge_msg::merge_message(repo, &commit_encoding)).flatten();
    let gitflow = crate::gitflow::read_config(repo);
    let lfs_patterns = crate::lfs::patterns_in(repo, workdir);
    BoxReads { head_message, commit_template, commit_encoding, merge_message, gitflow, lfs_patterns }
}

/// A load's read of the working tree's status, with what it took into account.
struct StatusRead {
    trust_scan: crate::trust::TrustScan,
    trust_print: crate::trust::TrustPrint,
    collisions: Vec<Collision>,
    index_memo: Option<std::sync::Arc<IndexMemo>>,
    unstaged: Vec<FileEntry>,
    staged: Vec<FileEntry>,
    slow_status: Option<SlowStatus>,
    changes: ChangeCounts,
}

/// Reads the status. Git's own status read can run the repository's fsmonitor hook or filters:
/// only once the user trusts it (and nothing was added since), or when nothing in it would run.
fn read_status(repo: &Repository, opts: &LoadOptions, cache: &GraphCache, workdir: &Path) -> Result<StatusRead> {
    let trust_scan = crate::trust::scan(repo);
    let trust_print = trust_scan.print();
    let git_status = trust_scan.is_empty() || (opts.trusted && opts.trusted_print.as_ref().is_some_and(|p| p.covers(&trust_print)));
    // Found once per load, before the status read that marks its rows with them.
    let collisions = tracing::debug_span!("load.collisions").in_scope(|| repo.index().map(|mut i| super::collide::collisions(&mut i)))?;
    let started = std::time::Instant::now();
    // Read before the status, so a change made during the read shows up in the next comparison.
    let index_memo = repo.index().ok().and_then(|i| super::IndexMemo::of(&i)).map(std::sync::Arc::new);
    let (unstaged, staged) = tracing::debug_span!("load.status")
        .in_scope(|| super::status::load_status_with(repo, git_status, &collisions, Some(cache.records())))?;
    let slow_status = super::status_speed::slow(started.elapsed(), workdir);
    let changes = ChangeCounts::of(&staged, &unstaged);
    Ok(StatusRead { trust_scan, trust_print, collisions, index_memo, unstaged, staged, slow_status, changes })
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
        out.push(commit_text::message(&repo.find_commit(oid?)?).into_owned());
    }
    Ok(out)
}
