//! Repository model built on libgit2.
//! Reads (history, refs, status, diffs) and index/working-tree edits happen here; operations
//! that should run hooks or talk to remotes go through `gitcli`.
//!
//! The types are here (the diff ones in `diff_types`); the work is split by area: `load`
//! (history, refs, status), `diff`, `history` (one file's commits), `index` and `lines`
//! (staging, line staging), `revert`, `restore`, `conflict` and `resolve`, `template`
//! (commit.template), `workfile`, `workfile_mv`, `workfile_rm`, `noclobber` and `removal`
//! (working-tree files), `tracked`, `attrs` (gitattributes), `submodules`, `worktrees`,
//! `remotes` and `shallow`.

use crate::graph::Layout;
use git2::{Oid, RepositoryState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod attrs;
mod conflict;
mod diff;
mod diff_types;
mod graph_cache;
mod history;
mod index;
mod lines;
mod load;
mod noclobber;
mod refs_key;
mod remotes;
mod removal;
mod resolve;
mod restore;
mod revert;
mod shallow;
mod status;
mod status_cli;
mod submodules;
mod template;
mod tracked;
mod trash;
mod walk;
mod workfile;
mod workfile_mv;
mod workfile_rm;
mod worktrees;

pub use conflict::{Chunk, ConflictContent, ConflictFile, ConflictIds, Operation, Segment, Side, SideLabels, load_conflict};
pub use diff::{
    Comparison, ViewDiff, commit_detail, commit_stats, commit_stats_until, compare, compare_files, load_diff, load_diff_for_view,
    load_diff_with,
};
use diff::{head_tree, path_opts, workdir_opts};
pub use diff_types::{DiffLine, DiffTarget, FileDiff, Hunk, HunkOp, LoadDiffOptions};
pub use graph_cache::GraphCache;
pub use history::{FileHistory, HistoryEntry, HistoryStart, file_history};
pub use index::{apply_hunk, discard_file, discard_untracked, stage_all, stage_file, unstage_paths};
pub use lines::apply_lines;
pub use load::{load, load_cached, messages_between};
pub use refs_key::{open_file_stamp, refs_key};
pub use remotes::RemoteInfo;
pub use removal::{Deleted, Removal, Trasher};
pub use resolve::{Choice, render_output, resolve_with_side, write_resolution};
pub use restore::{RestoredFrom, restore_file};
pub use revert::revert_hunk;
pub use status::status_at;
pub use submodules::{LEFT_BEHIND_MAX, SubmoduleInfo, SubmoduleState, submodules_at, submodules_if_any};
pub use template::{CommitTemplate, commit_template};
pub use tracked::tracked_paths;
pub use trash::{TrashProblem, TrashRefusal, os_trash};
pub(crate) use workfile::contained;
pub use workfile::{PathProblem, check_new_path, create_file, delete_file, rename_file};
pub use workfile_mv::rename_file_staged;
pub use workfile_rm::{delete_staged_new_file, unstage_gone_new_file};
pub use worktrees::{WorktreeInfo, worktrees_at};

#[derive(Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub summary: String,
    /// Message body flattened to one line (shown dimmed after the summary).
    pub body: String,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub parents: Vec<Oid>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
}

#[derive(Clone)]
pub struct RefLabel {
    pub name: String,
    /// Full ref name, e.g. refs/heads/main (empty for a detached HEAD label).
    pub full: String,
    pub kind: RefKind,
    pub is_head: bool,
}

/// A branch's configured upstream (`branch.<name>.remote` and `.merge`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tracking {
    /// The remote name, or "." when the upstream is a local branch.
    pub remote: String,
    /// The upstream branch name on that remote, without refs/heads/.
    pub merge: String,
}

impl Tracking {
    /// Whether the upstream is on a remote rather than a local branch.
    pub fn is_remote(&self) -> bool {
        self.remote != "."
    }

    /// "remote/merge", or just the branch name for a local upstream.
    pub fn display(&self) -> String {
        if self.is_remote() { format!("{}/{}", self.remote, self.merge) } else { self.merge.clone() }
    }
}

#[derive(Clone)]
pub struct BranchInfo {
    pub name: String,
    pub oid: Option<Oid>,
    pub is_head: bool,
    /// The upstream ref's short name (e.g. origin/x), when it exists.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// The configured upstream, present even when its remote-tracking ref is gone.
    pub tracking: Option<Tracking>,
    /// Full name of the resolved upstream ref, e.g. refs/remotes/origin/x; None when
    /// `upstream` is None.
    pub upstream_ref: Option<String>,
    /// Whether HEAD contains the branch tip, so deleting the branch loses no commits. True for
    /// the checked-out branch; false when HEAD is unborn. Worked out only for branches whose
    /// upstream is a remote-tracking ref (deleting both is what asks); false for the others.
    pub merged: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FileEntry {
    pub path: String,
    /// M, A, D, R, T, ? (untracked) or ! (conflicted: see `load_conflict`).
    pub status: char,
}

#[derive(Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub oid: Oid,
}

pub struct Snapshot {
    pub workdir: PathBuf,
    /// `workdir` resolved on disk when it was loaded (links, junctions and short 8.3 names
    /// followed), so callers can tell two paths to one repository apart without touching the
    /// disk again. `workdir` itself when it can't be resolved.
    pub real_workdir: PathBuf,
    /// The path `load` was given, when it names `workdir` under another spelling (a short 8.3
    /// name, a link, `/var` for `/private/var`): what a typed path, a clone's destination or a
    /// recent-list entry may still say. `None` when it was `workdir` itself or a folder inside it.
    pub opened_as: Option<PathBuf>,
    /// `workdir`'s identity on disk when it was loaded (`paths::FolderId`): what tells whether
    /// another path is this repository whatever its spelling, case included. None on a volume
    /// that gives no file ids.
    pub folder: Option<crate::paths::FolderId>,
    pub git_dir: PathBuf,
    pub name: String,
    /// (remote name, fetch URL)
    pub remote_urls: Vec<(String, String)>,
    pub head_branch: Option<String>,
    pub head_oid: Option<Oid>,
    /// HEAD's full commit message (used to pre-fill "amend").
    pub head_message: Option<String>,
    /// The commit.template text to prefill an empty draft with.
    pub commit_template: Option<CommitTemplate>,
    /// The gitflow.* settings (Preferences > Git Flow, palette Start entries).
    pub gitflow: crate::gitflow::GitFlowConfig,
    /// The LFS patterns of the root .gitattributes (empty without one); refreshed by full loads only.
    pub lfs_patterns: Vec<crate::lfs::LfsPattern>,
    pub state: RepositoryState,
    pub commits: Vec<CommitInfo>,
    pub row_of: HashMap<Oid, usize>,
    pub truncated: bool,
    pub refs: HashMap<Oid, Vec<RefLabel>>,
    pub locals: Vec<BranchInfo>,
    pub remotes: Vec<(String, Vec<(String, Oid)>)>,
    /// Every configured remote with its raw fetch and push URLs, in `remotes` order (also remotes
    /// without branches). `remote_urls` keeps libgit2's rewritten fetch URLs for forge detection.
    pub remote_info: Vec<RemoteInfo>,
    /// Each remote's default branch from `refs/remotes/<remote>/HEAD`, as (remote, branch
    /// without prefix), in `remotes` order; remotes without one are left out.
    pub remote_heads: Vec<(String, String)>,
    pub tags: Vec<(String, Oid)>,
    /// Tags that point at a tree or blob instead of a commit: not shown, but their names are taken.
    pub other_tags: Vec<String>,
    /// Names in `tags` whose ref points straight at the commit (no tag object): lightweight tags.
    pub lightweight_tags: HashSet<String>,
    pub stashes: Vec<StashInfo>,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
    /// The index's submodules sorted by path; refreshed by full loads and by status reloads when
    /// the repository has any (`submodules_if_any`).
    pub submodules: Vec<SubmoduleInfo>,
    /// The main worktree first, then linked ones by path; refreshed by full loads only.
    pub worktrees: Vec<WorktreeInfo>,
    /// What in this repository's own config and hooks runs programs; read at every full load.
    pub trust_scan: crate::trust::TrustScan,
    /// The commits where a shallow clone's history stops (their parents weren't downloaded), from the repository's shallow file; empty for a full clone. Refreshed by full loads only.
    pub shallow_roots: Vec<Oid>,
    pub graph: Layout,
    /// `refs_key` when the load started: a later equal key means only the status can differ.
    pub refs_key: u64,
    /// The options the snapshot was loaded with.
    pub loaded_with: LoadOptions,
}

impl Snapshot {
    /// Loaded commits reachable from `from` but not from `exclude` (newest first), e.g. the
    /// commits a branch would contribute to its target. Limited to the loaded history.
    pub fn commits_between(&self, from: Oid, exclude: Option<Oid>) -> Vec<&CommitInfo> {
        let reach = |start: Oid| {
            let mut seen = HashSet::new();
            let mut stack = vec![start];
            while let Some(o) = stack.pop() {
                if let Some(&row) = self.row_of.get(&o)
                    && seen.insert(o)
                {
                    stack.extend(self.commits[row].parents.iter().copied());
                }
            }
            seen
        };
        let excluded = exclude.map(reach).unwrap_or_default();
        let wanted = reach(from);
        self.commits.iter().filter(|c| wanted.contains(&c.oid) && !excluded.contains(&c.oid)).collect()
    }

    pub fn is_dirty(&self) -> bool {
        !self.unstaged.is_empty() || !self.staged.is_empty()
    }

    pub fn head_info(&self) -> Option<&BranchInfo> {
        self.locals.iter().find(|b| b.is_head)
    }

    /// Whether status says `path` is gone from the working tree: deleted there (unstaged 'D'),
    /// or deleted in the index ('D' staged) with nothing at the path since.
    pub fn missing_from_workdir(&self, path: &str) -> bool {
        if let Some(u) = self.unstaged.iter().find(|f| f.path == path) {
            return u.status == 'D';
        }
        self.staged.iter().any(|f| f.path == path && f.status == 'D')
    }
}

pub struct CommitDetail {
    pub oid: Oid,
    pub message: String,
    pub author: String,
    pub email: String,
    pub committer: String,
    pub time: i64,
    pub parents: Vec<Oid>,
    pub files: Vec<FileEntry>,
    /// Paths of `files` missing from the working tree when the detail was loaded (deleted,
    /// renamed or never checked out). Views use it to disable Open / Show in folder.
    pub absent: HashSet<String>,
    /// The format of the commit's signature, if it has one (header only; not checked).
    pub signature: Option<crate::signing::SigningFormat>,
}

impl CommitDetail {
    /// Whether `path` was in the working tree when the detail was loaded.
    pub fn in_workdir(&self, path: &str) -> bool {
        !self.absent.contains(path)
    }
}

/// Which history the graph shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadOptions {
    pub max_commits: usize,
    /// Only the checked-out branch, its upstream, the default branch(es) and `extra_tips`.
    pub smart: bool,
    /// Branch names (local or remote, e.g. PR source branches) kept visible in smart mode.
    pub extra_tips: Vec<String>,
    /// Full ref names to leave out of the graph.
    pub hidden: Vec<String>,
    /// When set, show only these full ref names (plus HEAD).
    pub solo: Vec<String>,
    /// The user trusts this repository: git may read its status even when the scan finds
    /// commands. Default false.
    pub trusted: bool,
}

impl LoadOptions {
    pub fn all(max_commits: usize) -> Self {
        Self { max_commits, ..Self::default() }
    }
}

#[cfg(test)]
mod compare_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod convert_write_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod discard_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod load_filter_tests;
#[cfg(test)]
mod refs_key_tests;
#[cfg(test)]
mod remotes_tests;
#[cfg(test)]
mod removal_tests;
#[cfg(test)]
mod resolve_eol_tests;
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod shallow_tests;
#[cfg(test)]
mod sparse_status_tests;
#[cfg(test)]
mod status_cli_tests;
#[cfg(test)]
mod submodules_tests;
#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tracked_tests;
#[cfg(test)]
mod walk_tests;
#[cfg(test)]
mod whole_tests;
#[cfg(test)]
mod workfile_exact_tests;
#[cfg(test)]
mod workfile_mv_tests;
#[cfg(test)]
mod workfile_rm_gone_tests;
#[cfg(test)]
mod workfile_rm_tests;
#[cfg(test)]
mod workfile_tests;
#[cfg(test)]
mod worktrees_tests;
