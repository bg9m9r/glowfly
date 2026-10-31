//! A loaded repository (`Snapshot`), the options it was loaded with (`LoadOptions`) and one
//! commit's details (`CommitDetail`), and the stamps of its file lists. `Snapshot`'s queries are in
//! `snapshot_impl`.

use super::*;
use crate::graph::Layout;
use git2::{Oid, RepositoryState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A stamp for a snapshot's file lists that no snapshot of this process has had
/// (`Snapshot::status_stamp`).
pub fn next_status_stamp() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A repository as a load read it: its history, refs, status and what the graph shows.
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
    /// The git dir every worktree of this repository shares: what groups worktree tabs.
    pub common: CommonDir,
    pub name: String,
    /// (remote name, fetch URL)
    pub remote_urls: Vec<(String, String)>,
    pub head_branch: Option<String>,
    pub head_oid: Option<Oid>,
    /// HEAD's full commit message (used to pre-fill "amend").
    pub head_message: Option<String>,
    /// The commit.template text to prefill an empty draft with.
    pub commit_template: Option<CommitTemplate>,
    /// MERGE_MSG cleaned of comment lines while a merge, cherry-pick or revert is stopped (prefills the commit
    /// draft); full loads only.
    pub merge_message: Option<String>,
    /// What commit messages are written in (`i18n.commitEncoding`); full loads only.
    pub commit_encoding: crate::commit_encoding::MessageEncoding,
    /// The gitflow.* settings (Preferences > Git Flow, palette Start entries).
    pub gitflow: crate::gitflow::GitFlowConfig,
    /// The LFS patterns of every attributes file (`lfs::repo_patterns`); refreshed by full loads and whole status reads (`LoadMsg`).
    pub lfs_patterns: Vec<crate::lfs::LfsPattern>,
    /// The operation in progress (`repo::state`: a pick or revert sequence with picks left
    /// counts, even once libgit2 says clean).
    pub state: RepositoryState,
    /// Where an in-progress rebase is (step, stopped commit); None otherwise. Read by every full
    /// load.
    pub rebase: Option<crate::rebase::RebaseProgress>,
    /// Where an in-progress `git am` stopped (patch, subject, conflicts); None otherwise. Read by
    /// every full load.
    pub am: Option<crate::patch_ops::AmProgress>,
    pub commits: std::sync::Arc<[CommitInfo]>,
    pub row_of: HashMap<Oid, usize>,
    pub truncated: bool,
    /// Where the history walk stopped when it was cut short: what a load for more goes on from
    /// (`repo::load_more`); None for a whole history.
    pub frontier: Option<std::sync::Arc<Frontier>>,
    /// The commits the history walk starts from, in the order pushed (HEAD, then the visible
    /// tips): with `shallow_roots`, what decides which commits a longer walk would add
    /// (`commit_index::IndexKey`).
    pub starts: Vec<Oid>,
    pub refs: HashMap<Oid, Vec<RefLabel>>,
    pub locals: Vec<BranchInfo>,
    pub remotes: Vec<(String, Vec<(String, Oid)>)>,
    /// Every configured remote with its raw fetch and push URLs, in `remotes` order (also remotes
    /// without branches). `remote_urls` keeps libgit2's rewritten fetch URLs for forge detection.
    pub remote_info: Vec<RemoteInfo>,
    /// Each remote's default branch from `refs/remotes/<remote>/HEAD`, as (remote, branch
    /// without prefix), in `remotes` order; remotes without one are left out.
    pub remote_heads: Vec<(String, String)>,
    /// `branch.<name>.pushRemote` and `remote.pushDefault` (`push_config`).
    pub push_remotes: PushRemotes,
    pub tags: Vec<(String, Oid)>,
    /// Tags that point at a tree or blob instead of a commit: not shown, but their names are taken.
    pub other_tags: Vec<String>,
    /// Names in `tags` whose ref points straight at the commit (no tag object): lightweight tags.
    pub lightweight_tags: HashSet<String>,
    pub stashes: Vec<StashInfo>,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
    /// The counts of `staged` and `unstaged`, kept with them.
    pub changes: ChangeCounts,
    /// Stamps `unstaged` and `staged`: a new number at every load and every status read that
    /// changes them in place (`next_status_stamp`), unique across snapshots, so equal stamps mean
    /// the same lists.
    pub status_stamp: u64,
    /// The index entries the staged list was read from; None when unknown.
    pub index_memo: Option<std::sync::Arc<IndexMemo>>,
    /// The status read took long (`SLOW_STATUS`); what could speed it up.
    pub slow_status: Option<SlowStatus>,
    /// What the work tree's file system treats as one name (probed in the git dir).
    pub folding: Folding,
    /// `core.ignorecase`.
    pub ignorecase: bool,
    /// Index paths that are one name to some file system (case or Unicode normalization), from
    /// full loads. Rows of the ones that share a file here carry `Hazard::Twins`.
    pub collisions: Vec<Collision>,
    /// The index's submodules sorted by path; refreshed by full loads and by status reloads when
    /// the repository has any (`submodules_if_any`).
    pub submodules: Vec<SubmoduleInfo>,
    /// The main worktree first, then linked ones by path; refreshed by full loads only.
    pub worktrees: Vec<WorktreeInfo>,
    /// What in this repository's own config and hooks runs programs; read at every full load.
    pub trust_scan: crate::trust::TrustScan,
    /// The print of `trust_scan` (`TrustScan::print`).
    pub trust_print: crate::trust::TrustPrint,
    /// The commits where a shallow clone's history stops (their parents weren't downloaded), from the repository's shallow file; empty for a full clone. Refreshed by full loads only.
    pub shallow_roots: Vec<Oid>,
    /// When this worktree's repository last fetched (FETCH_HEAD's time, Unix seconds), read by
    /// full loads; jobs that fetch set it too.
    pub fetched_at: Option<i64>,
    pub graph: Layout,
    /// Which local branches are stacked on which (full loads only).
    pub stacks: crate::stack::Stacks,
    /// `refs_key` when the load started: a later equal key means only the status can differ.
    pub refs_key: u64,
    /// The options the snapshot was loaded with.
    pub loaded_with: LoadOptions,
}

pub struct CommitDetail {
    pub oid: Oid,
    pub message: String,
    pub author: String,
    pub email: String,
    pub committer: String,
    pub committer_email: String,
    /// The committer date (seconds).
    pub time: i64,
    /// When the author wrote the change (seconds); `time` is when it was committed.
    pub author_time: i64,
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
    /// What the trust decision was made on: with `trusted`, git reads the status only while it
    /// covers the scan. None: a decision saved before prints were kept.
    pub trusted_print: Option<crate::trust::TrustPrint>,
    /// Count other worktrees' uncommitted changes and give each changed one a WIP row in the graph.
    pub worktree_wips: bool,
}

impl LoadOptions {
    pub fn all(max_commits: usize) -> Self {
        Self { max_commits, worktree_wips: true, ..Self::default() }
    }
}
