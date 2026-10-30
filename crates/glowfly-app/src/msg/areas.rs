//! The message areas: one row per `Msg` variant that carries an area's own enum, with the area's
//! short name and its handler in `update/` (and which of `now` and `fx` the handler takes). A new
//! area is one row here; `Msg`, `Msg::on_inactive`, `Msg::area` and `update`'s routing are built
//! from this table.
//!
//! `msg_areas! { then! { args } }` calls `then! { { args } rows }`, so each consumer names its own
//! callback: `msg_enum!` (below) declares `Msg` and its per-area methods, and `area_router!`
//! (update/mod.rs) routes each message to its handler. The short name is what the frame timing
//! (`state/frame_times.rs`) and the `update` tracing span call the time an area spent.

/// Calls `$then!` with its own arguments in braces, then the area rows.
macro_rules! msg_areas {
    ($then:ident ! { $($args:tt)* }) => {
        $then! { { $($args)* }
            /// Opening the repository, selecting a commit or the working tree, the open diff (update/repo.rs).
            Repo(crate::msg::RepoMsg) "repo" => repo::handle(now, fx);
            /// The recent and favorite repository lists (update/recent.rs).
            Recent(crate::msg::RecentMsg) "recent" => recent::handle(now, fx);
            /// Finding repositories in project folders (update/repos.rs).
            Repos(crate::msg::ReposMsg) "repos" => repos::handle(now, fx);
            /// Refresh results and loading more history (update/refresh.rs).
            Load(crate::msg::LoadMsg) "load" => refresh::handle(now, fx);
            /// The file watcher's reports (update/watch.rs).
            Watch(crate::msg::WatchMsg) "watch" => watch::handle(now, fx);
            /// Selecting several commits, comparing and cherry-picking them (update/selection.rs).
            Selection(crate::msg::SelectionMsg) "selection" => selection::handle(now, fx);
            /// Hidden and soloed refs and the hovered ref (update/graph_filter.rs).
            Graph(crate::msg::GraphMsg) "graph" => graph_filter::handle(fx);
            /// The graph search: query, filter, author picker (update/search.rs).
            Search(crate::msg::SearchMsg) "search" => search::handle(now, fx);
            /// Author avatars (update/avatars.rs).
            Avatars(crate::msg::AvatarMsg) "avatars" => avatars::handle(fx);
            /// The open diff's layout, folds, line selection and highlighting (update/diff.rs).
            Diff(crate::msg::DiffMsg) "diff" => diff::handle(fx);
            /// The conflict view (update/conflict.rs).
            Conflict(crate::msg::ConflictMsg) "conflict" => conflict::handle(fx);
            /// A file's history (update/history.rs).
            History(crate::msg::HistoryMsg) "history" => history::handle(now, fx);
            /// The blame view (update/blame.rs).
            Blame(crate::msg::BlameMsg) "blame" => blame::handle(now, fx);
            /// The interactive rebase panel (update/rebase.rs).
            Rebase(crate::msg::RebaseMsg) "rebase" => rebase::handle(now, fx);
            /// Pushing a stack and creating its pull requests (update/stack_push.rs).
            Stack(crate::msg::StackMsg) "stack" => stack_push::handle(now, fx);
            /// The tab bar (update/tabs.rs).
            Tabs(crate::msg::TabsMsg) "tabs" => tabs::handle(fx);
            /// Staging, unstaging and discarding files, hunks and lines (update/ops.rs).
            Stage(crate::msg::StageMsg) "stage" => ops::stage(now, fx);
            /// The commit box (update/commit.rs).
            Commit(crate::msg::CommitMsg) "commit" => commit::handle(now, fx);
            /// Git jobs: commands, push, stash, undo and redo, and their end (update/ops.rs).
            Job(crate::msg::JobMsg) "job" => ops::job(now, fx);
            /// Delete lock file after a job failed on a lock git left behind (update/lock_file.rs).
            Lock(crate::msg::LockMsg) "lock" => lock_file::handle(now, fx);
            /// Renaming a stash and the Stash dialog (update/stash.rs).
            Stash(crate::msg::StashMsg) "stash" => stash::handle(fx);
            /// Stash and carry on: the auto-stash prompt, putting it back and Show stash (update/auto_stash.rs).
            AutoStash(crate::msg::AutoStashMsg) "auto-stash" => auto_stash::handle(now, fx);
            /// Restoring a file from a commit, opening or showing a working-tree file (update/files.rs).
            Files(crate::msg::FilesMsg) "files" => files::handle(now, fx);
            /// Creating, renaming, deleting and discarding working-tree files (update/work_files.rs).
            WorkFile(crate::msg::WorkFileMsg) "work-file" => work_files::handle(now, fx);
            /// Branch menus and dialogs (update/branches.rs).
            Branch(crate::msg::BranchMsg) "branch" => branches::handle(now, fx);
            /// The Clone dialog (update/clone.rs).
            Clone(crate::msg::CloneMsg) "clone" => clone::handle(now, fx);
            /// The Init dialog (update/init.rs).
            Init(crate::msg::InitMsg) "init" => init::handle(now, fx);
            /// The New tag dialog and the TAGS section's remote push and delete (tag.rs).
            Tag(crate::tag::TagMsg) "tag" => tags::handle(fx);
            /// The REMOTE section and the Add/Edit remote dialog (remote.rs).
            Remote(crate::remote::RemoteMsg) "remote" => remotes::handle(fx);
            /// The WORKTREES section and Create worktree dialog (worktree.rs).
            Worktree(crate::worktree::WorktreeMsg) "worktree" => worktrees::handle(now, fx);
            /// The SUBMODULES section (submodule.rs).
            Submodule(crate::submodule::SubmoduleMsg) "submodule" => submodules::handle(now, fx);
            /// The built-in terminal panel (terminal.rs).
            Terminal(crate::terminal::TerminalMsg) "terminal" => terminal::handle(now, fx);
            /// Zoom and showing or hiding the left, details and toolbar panels (update/layout.rs).
            Layout(crate::msg::LayoutMsg) "layout" => layout::handle(now, fx);
            /// Saving commits as a patch file and applying one (update/patch.rs).
            Patch(crate::patch::PatchMsg) "patch" => patch::handle(now, fx);
            /// External editor and terminal (update/tools.rs).
            Tools(crate::tools::ToolsMsg) "tools" => tools::handle(now, fx);
            /// Custom commands from commands.toml files (update/custom_commands.rs).
            CustomCommand(crate::custom_commands::CustomCommandMsg) "custom-command" => custom_commands::handle(now, fx);
            /// Predicted merge conflicts of the current branch (update/conflict_check.rs).
            ConflictCheck(crate::conflict_check::ConflictCheckMsg) "conflict-check" => conflict_check::handle(now, fx);
            /// Previewing an operation before it runs (update/op_preview.rs).
            OpPreview(crate::msg::OpPreviewMsg) "op-preview" => op_preview::handle(now, fx);
            /// The operation journal: the states HEAD and the branches have been in, and Restore
            /// (update/journal.rs).
            Journal(crate::journal::JournalMsg) "journal" => journal::handle(now, fx);
            /// Comparing two versions of a branch commit by commit (update/range_diff.rs).
            RangeDiff(crate::range_diff::RangeDiffMsg) "range-diff" => range_diff::handle(now, fx);
            /// The pull request panel (update/pull_request/).
            PullRequest(crate::pull_request::PullRequestMsg) "pull-request" => pull_request::handle(now, fx);
            /// The issue panel and the Create issue dialog (update/issue.rs, update/issue_create.rs).
            Issue(crate::msg::IssueMsg) "issue" => issue::handle(now, fx);
            /// Hosting services: pull requests, issues, connections (update/forge.rs).
            Forge(crate::msg::ForgeMsg) "forge" => forge::handle(now, fx);
            /// PR and issue filters: header pickers and the saved-filter dialog (update/forge_filters.rs,
            /// update/forge_filter_edit.rs).
            ForgeFilter(crate::msg::FilterMsg) "forge-filter" => forge_filters::handle(fx);
            /// Publishing a repository with no remote (update/publish.rs).
            Publish(crate::msg::PublishMsg) "publish" => publish::handle(fx);
            /// Credential prompts from git and ssh (update/askpass.rs).
            Credentials(crate::msg::CredentialMsg) "credentials" => askpass::handle(now, fx);
            /// The Preferences window, pickers and signing keys (update/prefs.rs).
            Prefs(crate::msg::PrefsMsg) "prefs" => prefs::handle(now, fx);
            /// Preferences > Keyboard: recording, conflicts and resets (update/keys.rs).
            Keys(crate::msg::KeysMsg) "keys" => keys::handle(fx);
            /// The Git LFS preferences page (lfs.rs).
            Lfs(crate::lfs::LfsMsg) "lfs" => lfs::handle(now, fx);
            /// The Git Flow page and Start dialog (gitflow.rs).
            GitFlow(crate::gitflow::GitFlowMsg) "git-flow" => gitflow::handle(now, fx);
            /// The Sparse Checkout preferences page (update/sparse.rs).
            Sparse(crate::msg::SparseMsg) "sparse" => sparse::handle(now, fx);
            /// Preferences > SSH: key pairs and generating a key (ssh.rs).
            Ssh(crate::ssh::SshMsg) "ssh" => ssh::handle(now, fx);
            /// Named identity profiles: Preferences > Profile and switching (update/profiles.rs).
            Profile(crate::profile::ProfileMsg) "profile" => profiles::handle(now, fx);
            /// The command palette (update/palette.rs).
            Palette(crate::msg::PaletteMsg) "palette" => palette::handle(now, fx);
            /// The repository trust prompt (update/trust.rs).
            Trust(crate::msg::TrustMsg) "trust" => trust::handle(now, fx);
            /// Which git runs and what it can do; repositories git refuses for their owner
            /// (update/git_env.rs).
            GitEnv(crate::git_env::GitEnvMsg) "git-env" => git_env::handle(now, fx);
            /// Theme files: loading, reloading, the folder and new files (update/themes.rs).
            Theme(crate::themes::ThemeMsg) "theme" => themes::handle(now, fx);
            /// Activity, error and performance logs (update/logs.rs).
            Logs(crate::logs::LogsMsg) "logs" => logs::handle(now, fx);
            /// The first-run welcome and the tour of the main window (update/onboarding.rs).
            Onboarding(crate::onboarding::OnboardingMsg) "onboarding" => onboarding::handle(now, fx);
            /// glowfly:// links: copying, opening, their dialogs and the scheme registration
            /// (update/deep_link.rs).
            DeepLink(crate::deep_link::DeepLinkMsg) "deep-link" => deep_link::handle(now, fx);
            /// The cross-repository dashboard (update/dashboard.rs).
            Dashboard(crate::msg::DashboardMsg) "dashboard" => dashboard::handle(now, fx);
            /// Holding and closing toasts, and clicks on desktop notifications (update/notify.rs).
            Notify(crate::msg::NotifyMsg) "notify" => notify::handle(now, fx);
            /// A click on a dialog's backdrop or Esc, and the discard question (update/dialog.rs).
            Dialog(crate::msg::DialogMsg) "dialog" => dialog::handle();
            /// Quitting and the About dialog (update/app.rs).
            App(crate::msg::AppMsg) "app" => app::handle(fx);
            /// Glowfly's own updates and the release notes (update/updates.rs).
            Updates(crate::msg::UpdateMsg) "updates" => updates::handle(now, fx);
            /// Fetch full history's size check and a partial clone's missing contents (update/partial.rs).
            Partial(crate::msg::PartialMsg) "partial" => partial::handle(fx);
        }
    };
}

/// Declares `Msg` (the area rows, then the cross-cutting variants given as `variants`), with
/// `on_inactive` and `area` matching every area and then the arms given for the rest.
macro_rules! msg_enum {
    (
        {
            variants { $($extra:tt)* }
            on_inactive { $($inactive:tt)* }
            area { $($area:tt)* }
        }
        $( $(#[$doc:meta])* $V:ident($T:ty) $name:literal => $module:ident :: $func:ident ( $($arg:ident),* ); )*
    ) => {
        pub enum Msg {
            $( $(#[$doc])* $V($T), )*
            $($extra)*
        }

        impl Msg {
            /// What a result for a tab that is no longer active still needs (`Msg::Scoped`); `None`
            /// drops it, and the tab reloads when it is selected again. Each area decides for each of
            /// its messages.
            pub fn on_inactive(self) -> Option<Inactive> {
                match self {
                    $( Msg::$V(m) => m.on_inactive(), )*
                    $($inactive)*
                }
            }

            /// The area's short name, e.g. "repo" for `Msg::Repo`. A scoped message is named after the
            /// message inside it.
            pub fn area(&self) -> &'static str {
                match self {
                    $( Msg::$V(_) => $name, )*
                    $($area)*
                }
            }

            /// Every area's short name, in the table's order.
            #[cfg(test)]
            pub const AREAS: &'static [&'static str] = &[$($name),*];
        }
    };
}

pub(crate) use {msg_areas, msg_enum};

#[cfg(test)]
mod tests {
    use crate::msg::{Msg, RepoMsg};
    use std::path::PathBuf;

    #[test]
    fn area_names_are_unique_and_kebab_case() {
        let names = Msg::AREAS;
        assert_eq!(names.len(), 66, "one per area row");
        for (i, name) in names.iter().enumerate() {
            assert!(!name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'), "{name:?}");
            assert!(!name.starts_with('-') && !name.ends_with('-'), "{name:?}");
            assert!(!names[..i].contains(name), "{name} is named twice");
        }
    }

    #[test]
    fn a_scoped_message_is_named_after_the_one_inside() {
        assert_eq!(Msg::Repo(RepoMsg::WindowFocused).area(), "repo");
        let scoped = Msg::Scoped { workdir: PathBuf::from("w"), msg: Box::new(Msg::Repo(RepoMsg::WindowFocused)) };
        assert_eq!(scoped.area(), "repo");
        assert_eq!(Msg::Tick(std::time::Instant::now()).area(), "tick");
    }
}
