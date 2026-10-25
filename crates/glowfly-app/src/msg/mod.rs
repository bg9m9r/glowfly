//! Everything that can happen: user intents from views and results from background effects.
//!
//! `Msg` has one variant per area, each carrying that area's own enum (defined in this folder,
//! or in the area's module for `lfs`, `gitflow`, `worktree`, `remote`, `tag`, `submodule`,
//! `terminal`, `ssh`, `profile`, `patch`, `tools`, `custom_commands`, `conflict_check`, `journal`, `range_diff`, `pull_request`, `logs`, `onboarding` and `deep_link`;
//! Glowfly's own updates are `UpdateMsg` in `updates.rs`). Each area is one row of the table in
//! `areas.rs` (variant, enum, short name, handler), which builds `Msg`, `Msg::on_inactive`,
//! `Msg::area` and `update`'s routing. Each area's enum goes to one handler in `update/`, which
//! matches it exhaustively, and says through `on_inactive` what its results still need when they
//! arrive for a tab that is no longer active. Only a few cross-cutting messages are top-level.

use crate::effect::{FileOp, WorkFileOk};
use glowfly_core::gitcli::args;
use std::path::PathBuf;
use std::time::Instant;

mod app;
mod areas;
mod blame;
mod branch;
mod dashboard;
mod dialog;
mod diff;
mod files;
mod forge;
mod forge_filters;
mod graph;
mod init;
mod issue;
mod keys;
mod layout;
mod notify;
mod op_preview;
mod partial;
mod prefs;
mod rebase;
mod recent;
mod repo;
mod repos;
mod sparse;
mod stack;
mod stash;
mod tabs;
mod trust;
mod updates;
mod work;

pub use app::AppMsg;
pub(crate) use areas::msg_areas;
use areas::msg_enum;
pub use blame::BlameMsg;
pub use branch::{BranchMsg, CloneMsg};
pub use dashboard::DashboardMsg;
pub use dialog::{DialogMsg, Dismiss};
pub use diff::{ConflictMsg, DiffMsg, HistoryMsg};
pub use files::{FilesMsg, WorkFileMsg, not_opened, restore_failed};
pub use forge::{CredentialMsg, ForgeMsg, PublishMsg};
pub use forge_filters::FilterMsg;
pub use graph::{AvatarMsg, GraphMsg, PathsChunk, SearchMsg, SelectionMsg};
pub use init::InitMsg;
pub use issue::IssueMsg;
pub use keys::KeysMsg;
pub use layout::LayoutMsg;
pub use notify::NotifyMsg;
pub use op_preview::{OpPreviewMsg, ResetMode};
pub use partial::{ContentFailure, PartialMsg};
pub use prefs::{FilePurpose, FolderPurpose, PaletteMsg, PrefsMsg};
pub use rebase::RebaseMsg;
pub use recent::{FolderCheck, RecentMsg};
pub use repo::{LoadMsg, RepoMsg, WatchMsg};
pub use repos::{ReposMsg, ScanFound};
pub use sparse::SparseMsg;
pub use stack::StackMsg;
pub use stash::{AutoStashMsg, StashMsg};
pub use tabs::{CloseRequest, TabsMsg};
pub use trust::{InheritedRepo, TrustMsg};
pub use updates::{CheckedRelease, UpdateMsg};
pub use work::{CommitMsg, JobMsg, RevertWhat, StageMsg};

// The areas' variants come from the table in areas.rs; the cross-cutting messages, which are
// not one area's and are handled in update/mod.rs, are written here.
msg_areas! { msg_enum! {
    variants {
        /// A key from ui/shortcuts/global.rs: stage or unstage the open file or its selected lines, stage
        /// or unstage all, go to the commit message, or open or close the shortcut sheet. Checked
        /// and turned into the usual messages in update/shortcuts.rs.
        Shortcut(crate::keymap::KeyAction),

        // ---- Cross-cutting: not one area's, so handled in update/mod.rs.
        /// Once per frame: drives auto-fetch and expiring toasts.
        Tick(Instant),
        /// A result for the repository at `workdir`, applied only if that is still the active tab
        /// (otherwise see `on_inactive`). Wraps any area's result.
        Scoped {
            workdir: PathBuf,
            msg: Box<Msg>,
        },
        /// Ask before running `then`, from any area (`Msg::confirm`).
        Confirm {
            title: String,
            body: String,
            button: String,
            then: Box<Msg>,
        },
        /// Ask before running `then`, which loses work: drawn with a danger button
        /// (`Msg::confirm_danger`). With `ack`, a checkbox with that label must be ticked first
        /// (`Msg::confirm_danger_ack`), for a step that loses work for good.
        ConfirmDanger {
            title: String,
            body: String,
            button: String,
            ack: Option<String>,
            then: Box<Msg>,
        },
        ConfirmAccepted,
        /// The alternative button of `Modal::ConfirmChoice`: runs its `alt_then`.
        ConfirmAlt,
        /// A toast's action button (state/toasts.rs): runs its message and removes the toast (update/notify.rs).
        ToastAction(u64),
        /// A toast's Details beside its fix (state/toasts.rs): removes the toast and shows the
        /// activity log.
        ToastDetails(u64),
        /// Close whichever dialog is open.
        CloseModal,
        // The outside world, from menus all over the app.
        Copy(String),
        OpenUrl(String),
        OpenTerminal,
        OpenFolder(PathBuf),
        /// The activity log's Clear button.
        ClearLog,
        /// A line for the activity log from a background effect of any area.
        Logged {
            ok: bool,
            text: String,
        },
    }
    on_inactive {
        Msg::Tick(_)
        | Msg::Shortcut(_)
        | Msg::Confirm { .. }
        | Msg::ConfirmDanger { .. }
        | Msg::ConfirmAccepted
        | Msg::ConfirmAlt
        | Msg::ToastAction(_)
        | Msg::ToastDetails(_)
        | Msg::CloseModal
        | Msg::Copy(_)
        | Msg::OpenUrl(_)
        | Msg::OpenTerminal
        | Msg::OpenFolder(_)
        | Msg::Scoped { .. }
        | Msg::ClearLog => None,
        // Every runner line is a failure; it belongs to that tab's log.
        Msg::Logged { ok, text } => (!ok).then_some(Inactive::Log(text)),
    }
    area {
        Msg::Shortcut(_) => "shortcut",
        Msg::Tick(_) => "tick",
        Msg::Scoped { msg, .. } => msg.area(),
        Msg::Confirm { .. } | Msg::ConfirmDanger { .. } | Msg::ConfirmAccepted | Msg::ConfirmAlt | Msg::CloseModal => "confirm",
        Msg::ToastAction(_) | Msg::ToastDetails(_) => "toast",
        Msg::Copy(_) | Msg::OpenUrl(_) | Msg::OpenTerminal | Msg::OpenFolder(_) => "outside",
        Msg::ClearLog | Msg::Logged { .. } => "log",
    }
} }

/// What a result for a tab that is no longer active still needs (`Msg::on_inactive`).
pub enum Inactive {
    /// A failure for that tab's activity log.
    Log(String),
    /// An undoable operation: its undo belongs to the history of the tab it ran in.
    Undo(glowfly_core::undo::Action),
    /// A failure for that tab's activity log that still left an undoable change (both of the above).
    LogAndUndo { text: String, undo: glowfly_core::undo::Action },
    /// A working-tree file operation's result: a dialog may still be waiting for it.
    WorkFile { op: FileOp, result: Result<WorkFileOk, String> },
    /// A whole-history index build for that tab finished: the parked tab keeps the index.
    IndexBuilt { build: u64, result: Result<std::sync::Arc<glowfly_core::commit_index::CommitIndex>, String> },
}

impl Msg {
    pub fn git(label: &str, a: &[&str]) -> Msg {
        Msg::Job(JobMsg::Git { label: label.into(), cmds: vec![args(a)] })
    }

    pub fn confirm(title: &str, body: String, button: &str, then: Msg) -> Msg {
        Msg::Confirm { title: title.into(), body, button: button.into(), then: Box::new(then) }
    }

    pub fn confirm_danger(title: &str, body: String, button: &str, then: Msg) -> Msg {
        Msg::ConfirmDanger { title: title.into(), body, button: button.into(), ack: None, then: Box::new(then) }
    }

    /// `confirm_danger` whose button waits for the acknowledgement `ack` to be ticked. For views;
    /// `update` code builds the Modal with `Modal::danger(..).with_ack(..)` instead.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn confirm_danger_ack(title: &str, body: String, button: &str, ack: &str, then: Msg) -> Msg {
        Msg::ConfirmDanger { title: title.into(), body, button: button.into(), ack: Some(ack.into()), then: Box::new(then) }
    }
}
