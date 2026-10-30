//! A cheap fingerprint of everything a full load reads besides the working-tree status: refs,
//! HEAD, stashes, the repository state, config (the shared one and the worktree's own), linked
//! worktrees and every worktree's HEAD and the branch it is rebasing or bisecting, a shallow clone's `shallow` file and the trust print of the repository's own
//! settings, hooks and commands file (`trust::scan_top`). When it hasn't changed since the last
//! load, re-reading the status is enough and the history needn't be walked again.

use crate::error::Result;
use git2::Repository;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// The fingerprint of the repository at `workdir` (see the module docs). Only meaningful when
/// compared with another taken in the same run of the app.
pub fn refs_key(workdir: &Path) -> Result<u64> {
    key_of(&*crate::repo_pool::lend(workdir, crate::repo_pool::Use::Refs)?)
}

pub(super) fn key_of(repo: &Repository) -> Result<u64> {
    let now = SystemTime::now();
    let file_stamp = |p: &Path| file_stamp(p, now);
    let mut h = DefaultHasher::new();
    (super::op_state::state(repo) as u32).hash(&mut h);
    match repo.find_reference("HEAD") {
        Ok(head) => {
            head.symbolic_target_bytes().hash(&mut h);
            head.target().hash(&mut h);
            repo.head().ok().and_then(|r| r.target()).hash(&mut h);
        }
        Err(_) => 0u8.hash(&mut h),
    }
    for r in repo.references()? {
        let r = r?;
        // Glowfly's own records (kept discards and aborts) are never part of a load.
        if r.name_bytes().starts_with(b"refs/glowfly/") {
            continue;
        }
        r.name_bytes().hash(&mut h);
        r.target().hash(&mut h);
        r.symbolic_target_bytes().hash(&mut h);
    }
    let common = repo.commondir();
    // Dropping stash@{1} only rewrites the stash's reflog.
    file_stamp(&common.join("logs/refs/stash")).hash(&mut h);
    // Remotes, upstreams, gitflow and the commit template.
    file_stamp(&common.join("config")).hash(&mut h);
    // The worktree's own config (`extensions.worktreeConfig`) can set remotes and the commands
    // git runs too. The main worktree's sits in the common dir, a linked one's in its git dir.
    file_stamp(&repo.path().join("config.worktree")).hash(&mut h);
    // A shallow clone deepened or completed: `fetch --unshallow` may leave every ref as it was.
    file_stamp(&common.join("shallow")).hash(&mut h);
    // `git am --skip` stopping at the next patch moves no ref.
    file_stamp(&repo.path().join("rebase-apply/next")).hash(&mut h);
    // What makes git run programs (the settings, included files too, each hook with its script,
    // the commands file): a change made outside Glowfly loads again, and the trust check asks.
    crate::trust::scan_top(repo).print().0.hash(&mut h);
    // git status runs git in each checked-out submodule, which reads its own config and hooks:
    // stamped here, so a change loads in full and the full trust scan sees it.
    submodule_stamps(repo, now, &mut h);
    // The root .gitattributes' LFS patterns aren't in it: every whole status read brings them.
    // The main worktree's HEAD and the branch it is rebasing or bisecting, seen from a linked
    // one (they are the current worktree's own otherwise, and hashed above too).
    std::fs::read(common.join("HEAD")).ok().hash(&mut h);
    super::op_state::held_branch(common).hash(&mut h);
    // Linked worktrees: added, removed, locked, switched to another branch, or rebasing or
    // bisecting one.
    if let Ok(dir) = std::fs::read_dir(common.join("worktrees")) {
        let mut names: Vec<_> = dir.flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            p.hash(&mut h);
            std::fs::read(p.join("HEAD")).ok().hash(&mut h);
            let gitdir = std::fs::read_to_string(p.join("gitdir")).ok();
            // A worktree whose folder is gone shows as missing.
            gitdir.as_deref().map(|g| Path::new(g.trim()).exists()).hash(&mut h);
            gitdir.hash(&mut h);
            p.join("locked").exists().hash(&mut h);
            super::op_state::held_branch(&p).hash(&mut h);
        }
    }
    Ok(h.finish())
}

/// Stamps the config files and hooks of the submodules' git dirs: those under the common dir's
/// `modules/` (any depth: names may hold `/`, and nested submodules sit in `modules/` of their
/// parent's), and the `.git` folders of submodules `.gitmodules` lists that keep their own.
/// Only stats and small reads; never opens a repository.
fn submodule_stamps(repo: &Repository, now: SystemTime, h: &mut DefaultHasher) {
    modules_stamps(&repo.commondir().join("modules"), 0, now, h);
    let Some(workdir) = repo.workdir() else { return };
    let Ok(cfg) = git2::Config::open(&workdir.join(".gitmodules")) else { return };
    let Ok(entries) = cfg.entries(Some(r"^submodule\..*\.path$")) else { return };
    let mut paths = Vec::new();
    let _ = entries.for_each(|e| paths.extend(e.value().map(str::to_owned)));
    for p in paths {
        let dot_git = workdir.join(&p).join(".git");
        if std::fs::symlink_metadata(&dot_git).is_ok_and(|m| m.is_dir()) {
            p.hash(h);
            git_dir_stamps(&dot_git, now, h);
        }
    }
}

/// How deep `modules_stamps` looks.
const MAX_MODULES_DEPTH: usize = 24;

fn modules_stamps(dir: &Path, depth: usize, now: SystemTime, h: &mut DefaultHasher) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    if depth >= MAX_MODULES_DEPTH {
        return;
    }
    let mut dirs: Vec<_> = read.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        d.hash(h);
        if d.join("config").is_file() {
            git_dir_stamps(&d, now, h);
            modules_stamps(&d.join("modules"), depth + 1, now, h);
        } else {
            // One part of a name like `libs/lib`.
            modules_stamps(&d, depth + 1, now, h);
        }
    }
}

/// One git dir's config files and the hooks in its hooks folder (`*.sample` left out).
fn git_dir_stamps(git_dir: &Path, now: SystemTime, h: &mut DefaultHasher) {
    file_stamp(&git_dir.join("config"), now).hash(h);
    file_stamp(&git_dir.join("config.worktree"), now).hash(h);
    let Ok(read) = std::fs::read_dir(git_dir.join("hooks")) else { return };
    let mut hooks: Vec<_> = read.flatten().map(|e| e.path()).filter(|p| p.extension().is_none_or(|x| x != "sample")).collect();
    hooks.sort();
    for p in hooks {
        p.hash(h);
        // Never read anything but a plain file: reading a FIFO blocks.
        if std::fs::metadata(&p).is_ok_and(|m| m.is_file()) {
            file_stamp(&p, now).hash(h);
        }
    }
}

/// A fingerprint of the working-tree file at `path` (relative to `workdir`) and of the index:
/// while it stays the same, a working-tree or index diff of that file can't have changed.
/// Taken before reading the status, so a change made meanwhile shows up next time.
pub fn open_file_stamp(workdir: &Path, path: &str) -> Option<u64> {
    let repo = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir).ok()?;
    let now = SystemTime::now();
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    file_stamp(&workdir.join(path), now).hash(&mut h);
    file_stamp(&repo.path().join("index"), now).hash(&mut h);
    Some(h.finish())
}

/// How recently before a stamp is taken a file may have been modified for its size and time
/// to still prove it unchanged later: a rewrite of the same length within one tick of a coarse
/// clock (FAT: 2 s; HFS+ and ext3: 1 s; a network file system's attribute cache) leaves both as
/// they were. git's "racy git" rule.
pub const RACY_WINDOW: Duration = Duration::from_secs(2);

/// Files up to this size are fingerprinted by their bytes, which costs little and needs no
/// trust in timestamps.
const HASHED_UP_TO: u64 = 64 * 1024;

/// Whether a file last modified at `modified` can be stamped by size and time at `taken_at`: it
/// was modified at least [`RACY_WINDOW`] earlier (not later: the clock may have been set back).
pub fn stamp_trustworthy(modified: Option<SystemTime>, taken_at: SystemTime) -> bool {
    modified.is_some_and(|m| taken_at.duration_since(m).is_ok_and(|age| age >= RACY_WINDOW))
}

/// A fingerprint of the file at `p` at `taken_at`.
#[derive(Hash)]
enum Stamp {
    Missing,
    /// A small file's length and the hash of its bytes.
    Bytes(u64, u64),
    /// A larger file's length and modification time, old enough to trust.
    Settled(u64, SystemTime),
    /// A larger file modified too recently to trust its stamp: differs from every other stamp
    /// (the time it was taken), so the next comparison reads it again.
    Unsettled(u128),
}

fn file_stamp(p: &Path, taken_at: SystemTime) -> Stamp {
    let Ok(m) = std::fs::metadata(p) else { return Stamp::Missing };
    if m.len() <= HASHED_UP_TO
        && let Ok(bytes) = std::fs::read(p)
    {
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        return Stamp::Bytes(bytes.len() as u64, h.finish());
    }
    match m.modified() {
        Ok(t) if stamp_trustworthy(Some(t), taken_at) => Stamp::Settled(m.len(), t),
        _ => Stamp::Unsettled(taken_at.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()),
    }
}
