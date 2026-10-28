//! The effect areas: one row per `Effect` variant that carries an area's own effect type. A new
//! area is one row here, an `EffectMeta` impl beside its type and an `Execute` impl in
//! `runner/<area>.rs`; `Effect` and every match over the areas are built from this table.
//!
//! `effect_areas! { then! { args } }` calls `then! { { args } rows }`, so each consumer names
//! its own callback: `effect_enum!` declares `Effect` (the args are its flat variants), and
//! `effect_match!` matches an effect, running one body for every area (each copy is checked
//! against that area's type) and the args' arms for the flat variants.

/// Calls `$then!` with its own arguments in braces, then the area rows.
macro_rules! effect_areas {
    ($then:ident ! { $($args:tt)* }) => {
        $then! { { $($args)* }
            /// Reading, checking and removing submodules (submodule.rs).
            Submodule(crate::effect::SubmoduleEffect),
            /// Counting the commits deleting a branch would lose, for the Delete dialog (branch.rs); reads only.
            Branch(crate::effect::BranchEffect),
            /// Stash list jobs: renaming an entry, putting an auto-stash back (stash.rs).
            Stash(crate::effect::StashEffect),
            /// A cherry-pick or revert job, carried past the commits that change nothing; unscoped like RunGit.
            Pick(crate::effect::PickJob),
            /// Interactive rebase: the panel's list, graph edits, running and resuming.
            Rebase(crate::effect::RebaseEffect),
            /// Pushing a stack's layers (stack.rs).
            Stack(crate::effect::StackEffect),
            /// Connected accounts: signing in, tokens, creating a repository, owners and boards (account.rs); unscoped.
            Account(crate::effect::AccountEffect),
            /// Per-file line counts of a commit or the working tree (runner/line_counts.rs); reads only.
            LineCounts(crate::effect::LineCountsEffect),
            /// The whole-history search index: build, query, forget (index.rs). Handled by Runner::run's
            /// index worker: one build at a time across tabs, newest request first, and one query.
            Index(crate::effect::IndexEffect),
            /// Whole-history work off the UI thread: relayout, ancestry, freeing a snapshot (background.rs).
            Background(crate::effect::BackgroundEffect),
            /// Hands something to the operating system: a browser, a terminal, the file manager, a
            /// picker (os.rs, runner/os.rs); unscoped, never touches the repository.
            Os(crate::effect::OsEffect),
            /// Git config values and signing keys for Preferences (prefs.rs); unscoped.
            Prefs(crate::effect::PrefsEffect),
            /// Listing and generating SSH keys (ssh.rs); unscoped, outside any repository.
            Ssh(crate::ssh::SshEffect),
            /// Saving or dropping an HTTPS sign-in typed in the prompt (runner/credentials.rs); unscoped.
            Credential(crate::effect::CredentialEffect),
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
            OpPreview(crate::effect::OpPreviewJob),
            /// Reading the operation journal and a state's preview (journal.rs); reads only.
            Journal(crate::journal::JournalEffect),
            /// Comparing two versions of a branch with git range-diff (range_diff.rs); reads only.
            RangeDiff(crate::range_diff::RangeDiffJob),
            /// Fork lists and forking for the REMOTE section (remote.rs); unscoped.
            Remote(crate::remote::RemoteEffect),
            /// A sparse clone, listing the user's repositories and checking the parent folder for the
            /// Clone dialog; unscoped.
            Clone(crate::effect::CloneEffect),
            /// The Init dialog's folder checks and the init job; unscoped.
            Init(crate::effect::InitEffect),
            /// A desktop notification (runner/notify.rs); unscoped.
            Notify(crate::effect::DesktopNote),
            /// Log files: the activity log, reading the error and performance logs, extended logging (runner/logs.rs).
            Logs(crate::logs::LogsEffect),
            /// The pull request panel's file list (scoped) and service calls (unscoped; pull_request.rs).
            PullRequest(crate::pull_request::PrEffect),
            /// Building and finding glowfly:// links, and the scheme registration (deep_link.rs); unscoped.
            DeepLink(crate::deep_link::DeepLinkEffect),
            /// The cross-repository dashboard (runner/dashboard.rs); unscoped; reads repositories with libgit2 and asks services.
            Dashboard(crate::effect::DashboardEffect),
            /// The issue panel's and the Create issue dialog's calls to the tracker (unscoped; issue.rs).
            Issue(crate::effect::IssueEffect),
            /// Update checks, downloads, Install and restart and tidying (runner/updates.rs); unscoped.
            Updates(crate::effect::UpdateEffect),
            /// Finding repositories in project folders (runner/repos.rs); unscoped, reads folder listings only.
            Repos(crate::effect::ReposEffect),
            /// Reads repositories just made from a trusted one, to trust them (runner/trust.rs; unscoped).
            Trust(crate::effect::TrustEffect),
            /// A shallow clone's size check and a partial clone's missing contents (runner/partial.rs); unscoped.
            Partial(crate::effect::partial::PartialEffect),
            /// Effects that need the window, carried out by the UI shell.
            Ui(crate::effect::UiEffect),
            /// Blames one file (`git blame`), reported as `BlameMsg::Loaded`.
            LoadBlame(crate::effect::BlameRequest),
            /// A file as of a commit, with syntax spans, for the history's File view (`FileTextLoaded`).
            LoadFileText(crate::effect::FileTextRequest),
            /// The worktree dialogs' folder check, removing and moving a worktree, and counting other
            /// worktrees' changes (worktree.rs).
            Worktree(crate::effect::WorktreeEffect),
            /// Whether git-lfs is installed, and the preferences page's Git LFS status (lfs.rs).
            Lfs(crate::effect::LfsEffect),
            /// The preferences page's sparse checkout state, and `git sparse-checkout` (sparse.rs).
            Sparse(crate::effect::SparseEffect),
            /// Loading a conflicted file and counting the conflict markers of working files (conflict.rs).
            Conflict(crate::effect::ConflictEffect),
            /// A selection's comparisons and stacked changes (selection.rs).
            Selection(crate::effect::SelectionEffect),
            /// Git's file system monitor started and set, then the cache's commands (monitor.rs); unscoped like RunGit.
            TurnOnMonitor(crate::effect::MonitorJob),
            /// Another effect with the trust re-check the runner makes before it starts it
            /// (effect/trust_check.rs, runner/trust.rs); its metadata is that effect's.
            Checked(Box<crate::effect::Checked>),
        }
    };
}

/// Declares `Effect`: one variant per area row, then the flat variants given in braces.
macro_rules! effect_enum {
    ({ $($flat:tt)* } $( $(#[$doc:meta])* $V:ident($T:ty), )*) => {
        #[derive(Debug)]
        pub enum Effect {
            $( $(#[$doc])* $V($T), )*
            $($flat)*
        }
    };
}

/// `match $value { Effect::<Area>($e) => $body, …, <flat arms> }`: `$body` runs once per area,
/// with `$e` bound to that area's value.
macro_rules! effect_match {
    ({ $value:expr, |$e:ident| $body:expr, { $($flat:tt)* } } $( $(#[$doc:meta])* $V:ident($T:ty), )*) => {
        match $value {
            $( crate::effect::Effect::$V($e) => $body, )*
            $($flat)*
        }
    };
}

pub(crate) use {effect_areas, effect_enum, effect_match};
