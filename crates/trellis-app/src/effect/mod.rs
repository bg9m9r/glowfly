//! Side effects requested by `update`. The runner executes them off the UI thread and reports
//! back with `Msg`s; the few that need the window (`UiEffect`) are handled by the UI shell.

pub use crate::file_op::{DeleteMode, FileOp, WorkFileOk};
use git2::Oid;
use std::path::PathBuf;
use trellis_core::askpass::Reply;
use trellis_core::forge::{Auth, Target};
use trellis_core::repo::{DiffTarget, FileDiff, HistoryStart, LoadOptions};
use trellis_core::settings::Settings;
use trellis_core::undo::{Action, UndoKind};

mod account;
mod blame;
mod clone;
mod credential;
mod dashboard;
mod forge;
mod forge_filters;
mod history;
mod init;
mod issue;
mod line_counts;
mod names;
mod notify;
mod op_preview;
mod os;
pub mod partial;
mod pick;
mod prefs;
mod rebase;
mod repo;
mod repos;
mod scope;
mod selection;
mod stack;
mod stash;
mod submodule;
mod trust;
mod ui;
mod updates;

pub use account::AccountEffect;
pub use blame::BlameRequest;
pub use clone::CloneEffect;
pub use credential::CredentialEffect;
pub use dashboard::*;
pub use forge::{ForgeCall, ForgeError, ForgeReply, ForgeResult};
pub use forge_filters::{FilterCall, FilterResult};
pub use history::FileTextRequest;
pub use init::InitEffect;
pub use issue::*;
pub use line_counts::{FailedLineCounts, LineCountsEffect};
pub use notify::{DesktopNote, NoteKind};
pub use op_preview::OpPreviewJob;
pub use os::OsEffect;
pub use pick::PickJob;
pub use prefs::PrefsEffect;
pub use rebase::{RebaseEffect, RestackJob};
pub use repo::{ConflictRead, KnownRepo, LoadKind, LocalOp, StatusRead, UndoOpts, UndoResult, WatchTarget};
pub use repos::ReposEffect;
pub use selection::StackRequest;
pub use stack::{StackEffect, StackPushJob};
pub use stash::{PutBackRequest, StashEffect};
pub use submodule::SubmoduleEffect;
pub use trust::TrustEffect;
pub use ui::UiEffect;
pub use updates::UpdateEffect;

#[derive(Debug)]
pub enum Effect {
    LoadRepo {
        path: PathBuf,
        opts: LoadOptions,
        generation: u64,
        kind: LoadKind,
        /// The snapshot on screen: when its `refs_key` still matches, only the status is read
        /// (`LoadMsg::RepoUnchanged`).
        known: Option<KnownRepo>,
    },
    LoadStatus {
        workdir: PathBuf,
        generation: u64,
        /// The last load found submodules: read them again even without a .gitmodules.
        submodules: bool,
        /// The file whose working-tree or index diff is open, for `LoadMsg::StatusLoaded::open_stamp`.
        open: Option<String>,
        /// Let git read the status (fsmonitor, untracked cache); false while the repository
        /// isn't trusted.
        git_status: bool,
        /// Re-read only these work-tree paths' unstaged rows; None reads everything.
        paths: Option<Vec<String>>,
    },
    LoadDetail {
        workdir: PathBuf,
        oid: Oid,
    },
    /// The files that differ from commit `from` to commit `to`.
    LoadCompare {
        workdir: PathBuf,
        from: Oid,
        to: Oid,
    },
    /// Each of these commits' own changes (`SelectionMsg::StackedLoaded`).
    LoadStacked(StackRequest),
    /// The files that differ from commit `oid` to the working tree.
    LoadWorkdirCompare {
        workdir: PathBuf,
        oid: Oid,
        generation: u64,
        /// From the working tree to the commit instead.
        swapped: bool,
    },
    LoadDiff {
        workdir: PathBuf,
        target: DiffTarget,
        reload: bool,
        generation: u64,
        /// On a reload, the diff on screen: unchanged hunks keep its highlighting.
        previous: Option<std::sync::Arc<FileDiff>>,
        /// Hide whitespace-only changes (the diff_ignore_whitespace setting when the request was made).
        ignore_whitespace: bool,
        /// Also load the whole file (the diff_mode setting when the request was made).
        whole_file: bool,
    },
    /// Finishes the syntax highlighting `diff` arrived with (pending in its hunks or its whole
    /// file) for the diff shown at `generation` (AppState::diff_shown_gen). Started by
    /// Runner::run with a cancel ticket: a newer HighlightDiff cancels this one.
    HighlightDiff {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
        diff: std::sync::Arc<FileDiff>,
    },
    /// Reads the unchanged lines between `diff`'s hunks for the hunk view, for the diff shown at
    /// `generation` (AppState::diff_shown_gen).
    LoadDiffGaps {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
        diff: std::sync::Arc<FileDiff>,
    },
    /// Reads the sizes and decodes the images of the binary diff shown at `generation`
    /// (AppState::diff_shown_gen), for its header and preview.
    LoadBinarySides {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
    },
    LoadConflict {
        workdir: PathBuf,
        path: String,
        generation: u64,
    },
    LocalOp {
        workdir: PathBuf,
        op: LocalOp,
    },
    /// Counts the conflict markers in the working files of `paths` (and, for a summary, lists
    /// the resolved files): only reads.
    ReadConflictFiles {
        workdir: PathBuf,
        paths: Vec<String>,
        purpose: ConflictRead,
    },
    /// Reverts part of a commit or comparison diff (a hunk, some of its lines, or the whole
    /// file) in the working tree only (trellis_core::repo::revert).
    Revert {
        workdir: PathBuf,
        req: trellis_core::repo::RevertRequest,
    },
    /// Writes `path` from commit `oid` (or its parent) into the working tree only.
    /// Without `overwrite`, refuses when the file's current contents would be lost.
    RestoreFile {
        workdir: PathBuf,
        oid: Oid,
        path: String,
        overwrite: bool,
    },
    /// Creates, renames, deletes or discards (untracked) a working-tree file; the index is untouched, except for
    /// UnstageDelete, UnstageGone (index only) and a staged rename.
    WorkFile {
        workdir: PathBuf,
        op: FileOp,
    },
    /// The files in the index, for the palette's file finder.
    LoadTrackedPaths {
        workdir: PathBuf,
        generation: u64,
    },
    /// Reading, checking and removing submodules (submodule.rs).
    Submodule(SubmoduleEffect),
    /// Checks the folder of a worktree about to be created (missing, or an empty folder) before
    /// `git worktree add` runs. Unscoped: the dialog waits for the answer even if tabs change.
    CheckWorktreeFolder(crate::worktree::NewWorktree),
    /// Whether the Clone dialog's parent folder `parent` exists, checked after `delay` so typing
    /// doesn't wait on the disk. Reported as `CloneMsg::ParentChecked`.
    CheckCloneParent {
        parent: String,
        delay: std::time::Duration,
    },
    /// Whether git-lfs is installed (`git lfs version`). Unscoped: the answer is global.
    DetectLfs {
        cwd: PathBuf,
    },
    /// `git worktree remove` then, if asked, `git branch -d`; unscoped like RunGit (ends the job).
    RemoveWorktree(crate::worktree::RemoveRequest),
    /// Other worktrees' changes the load left Pending (`WorktreeMsg::ChangesCounted`).
    CountWorktreeChanges(crate::worktree::CountRequest),
    /// `git worktree move` once the target is free; unscoped like RunGit (ends the job).
    MoveWorktree(crate::worktree::MoveRequest),
    /// Stash list jobs: renaming an entry, putting an auto-stash back (stash.rs).
    Stash(StashEffect),
    /// Git LFS status for the preferences page: installed version, set up or not, tracked patterns.
    LoadLfs {
        workdir: PathBuf,
        generation: u64,
    },
    /// Sparse checkout state for the preferences page; its paths only with `run_git` (trust).
    LoadSparse {
        workdir: PathBuf,
        generation: u64,
        run_git: bool,
    },
    /// `git sparse-checkout` set, reapply or disable; unscoped like RunGit (ends the job).
    SparseCheckout {
        cwd: PathBuf,
        command: trellis_core::sparse_ops::SparseCommand,
    },
    /// The commits that changed `path` (anything under it with `folder`), examining at most `limit`.
    LoadFileHistory {
        workdir: PathBuf,
        path: String,
        start: HistoryStart,
        limit: usize,
        folder: bool,
        generation: u64,
    },
    /// A file as of a commit, with syntax spans, for the history's File view (`FileTextLoaded`).
    LoadFileText(FileTextRequest),
    /// Blames one file (`git blame`), reported as `BlameMsg::Loaded`.
    LoadBlame(BlameRequest),
    /// Runs git commands in order, stopping at the first failure. `write_file` is written first
    /// (used for commit messages). With `record`, the state before and after is compared and
    /// the resulting undoable action reported back; with `journal`, a journal note is written
    /// when the run moved HEAD or the op's branch (runner/journal.rs). Without `prompts`
    /// (background jobs) git fails rather than open a credential prompt.
    RunGit {
        cwd: PathBuf,
        cmds: Vec<Vec<String>>,
        write_file: Option<(PathBuf, String)>,
        record: Option<UndoKind>,
        journal: Option<trellis_core::journal::Op>,
        prompts: bool,
        /// Run nothing unless this revision still resolves to this commit.
        expect: Option<(String, git2::Oid)>,
        /// Every command only fetches: `Effect::StopGit(StopKey::Job)` can stop it.
        stoppable: bool,
    },
    /// A cherry-pick or revert job, carried past the commits that change nothing; unscoped like RunGit.
    Pick(PickJob),
    /// Interactive rebase: the panel's list, graph edits, running and resuming.
    Rebase(RebaseEffect),
    /// Pushing a stack's layers (stack.rs).
    Stack(StackEffect),
    /// Undoes (or redoes) `action` after checking the repository still matches it. Unscoped like
    /// `RunGit`: the job's result must reach `update` even if its tab is no longer active.
    UndoRedo {
        cwd: PathBuf,
        action: Action,
        redo: bool,
        opts: UndoOpts,
    },
    Forge {
        target: Target,
        cwd: PathBuf,
        auth: Option<Auth>,
        interactive: bool,
        /// The user connected this account in Preferences (lets GCM sign in to a self-hosted server).
        connected: bool,
        call: ForgeCall,
    },
    /// Connected accounts: signing in, tokens, creating a repository, owners and boards
    /// (account.rs); unscoped.
    Account(AccountEffect),
    Stats {
        workdir: PathBuf,
        oids: Vec<Oid>,
    },
    /// Per-file line counts of a commit or the working tree (runner/line_counts.rs); reads only.
    LineCounts(LineCountsEffect),
    /// Which of `oids` (newest first) changed a path matching `terms`, for search request
    /// `request`; reported in chunks as SearchMsg::PathsSearched. Handled by Runner::run's
    /// path-search worker: a newer request replaces one waiting and stops one running.
    SearchPaths {
        workdir: PathBuf,
        request: u64,
        terms: Vec<trellis_core::path_query::PathTerm>,
        oids: Vec<Oid>,
    },
    /// Checks commit `oid`'s signature (in `format`) with git, for the details panel's badge.
    VerifySignature {
        workdir: PathBuf,
        oid: Oid,
        format: trellis_core::signing::SigningFormat,
        generation: u64,
    },
    /// Downloads (or reads from the disk cache) avatars one after another.
    FetchAvatars(Vec<String>),
    SaveSettings(Box<Settings>),
    SaveRecent(Vec<PathBuf>),
    /// Which of these repository folders are gone (not a directory or unreachable), and the
    /// identity of each that is there; reported as `RecentMsg::FoldersChecked`.
    CheckRepoFolders(Vec<PathBuf>),
    /// The common dir of each restored tab's folder, without loading it; reported as
    /// `TabsMsg::GroupsRead`.
    ReadTabGroups(Vec<PathBuf>),
    /// The open tabs (repositories), their labels (parallel to `paths`) and which one is
    /// active, for the next start.
    SaveTabs {
        paths: Vec<PathBuf>,
        labels: Vec<Option<String>>,
        active: usize,
    },
    /// Hands something to the operating system: a browser, a terminal, the file manager, a
    /// picker (os.rs, runner/os.rs); unscoped, never touches the repository.
    Os(OsEffect),
    /// Git config values and signing keys for Preferences (prefs.rs); unscoped.
    Prefs(PrefsEffect),
    /// Watch this repository for outside changes, replacing any previous watch; None stops
    /// watching. Handled by `Runner::run` itself (the watcher outlives a worker thread).
    Watch(Option<WatchTarget>),
    /// Answers credential prompt `id`. Handled by `Runner::run` itself, which owns the
    /// listener the helper is waiting on. The answer's `Debug` output is redacted.
    AnswerPrompt {
        id: u64,
        reply: Reply,
    },
    /// Listing and generating SSH keys (ssh.rs); unscoped, outside any repository.
    Ssh(crate::ssh::SshEffect),
    /// Saving or dropping an HTTPS sign-in typed in the prompt (runner/credentials.rs); unscoped.
    Credential(CredentialEffect),
    /// Writing a profile's identity to the global git config (profile.rs); unscoped.
    Profile(crate::profile::ProfileEffect),
    /// The built-in terminal panel's shells; handled by Runner::run, never a worker.
    Terminal(crate::terminal::TerminalEffect),
    /// Patch dialogs and saving commits as a patch (patch.rs); unscoped, each carries its repository.
    Patch(crate::patch::PatchEffect),
    /// Launching the external editor or a custom terminal, and their program pickers (tools.rs); unscoped.
    Tool(crate::tools::ToolEffect),
    /// Reading the commands files and running a shell command as the job (custom_commands.rs); unscoped.
    CustomCommand(crate::custom_commands::CustomCommandEffect),
    /// Finding git and its version, and adding a folder to safe.directory (git_env.rs); unscoped.
    GitEnv(crate::git_env::GitEnvEffect),
    /// Reading the themes folder, opening it and writing a new theme file (themes.rs); unscoped.
    Theme(crate::themes::ThemeEffect),
    /// The welcome's git identity read and write (runner/onboarding.rs); unscoped.
    Onboarding(crate::onboarding::OnboardingEffect),
    /// Dry-runs a merge with git merge-tree (conflict_check.rs); reads only.
    CheckConflicts(crate::conflict_check::CheckJob),
    /// Works out what an operation would do without changing the repository (runner/op_preview.rs); reads only.
    OpPreview(OpPreviewJob),
    /// Reading the operation journal and a state's preview (journal.rs); reads only.
    Journal(crate::journal::JournalEffect),
    /// Comparing two versions of a branch with git range-diff (range_diff.rs); reads only.
    RangeDiff(crate::range_diff::RangeDiffJob),
    /// Fork lists and forking for the REMOTE section (remote.rs); unscoped.
    Remote(crate::remote::RemoteEffect),
    /// A sparse clone and listing the user's repositories for the Clone dialog; unscoped.
    Clone(CloneEffect),
    /// The Init dialog's folder checks and the init job; unscoped.
    Init(InitEffect),
    /// A desktop notification (runner/notify.rs); unscoped.
    Notify(DesktopNote),
    /// Log files: the activity log, reading the error and performance logs, extended logging (runner/logs.rs).
    Logs(crate::logs::LogsEffect),
    /// The pull request panel's file list (scoped) and service calls (unscoped; pull_request.rs).
    PullRequest(crate::pull_request::PrEffect),
    /// Building and finding trellis:// links, and the scheme registration (deep_link.rs); unscoped.
    DeepLink(crate::deep_link::DeepLinkEffect),
    /// The cross-repository dashboard (runner/dashboard.rs); unscoped; reads repositories with
    /// libgit2 and asks services.
    Dashboard(DashboardEffect),
    /// The issue panel's and the Create issue dialog's calls to the tracker (unscoped; issue.rs).
    Issue(IssueEffect),
    /// Update checks, downloads and opening a downloaded installer (runner/updates.rs); unscoped.
    Updates(UpdateEffect),
    /// Finding repositories in project folders (runner/repos.rs); unscoped, reads folder listings only.
    Repos(ReposEffect),
    /// Reads repositories just made from a trusted one, to trust them (runner/trust.rs; unscoped).
    Trust(TrustEffect),
    /// A shallow clone's size check and a partial clone's missing contents (runner/partial.rs);
    /// unscoped.
    Partial(partial::PartialEffect),
    /// Stops the running git command registered under this key, if any. Handled by
    /// `Runner::run` itself, which holds the stops.
    StopGit(partial::StopKey),
    Ui(UiEffect),
}
