//! The area a message belongs to, by name: what the frame timing (`state/frame_times.rs`) and
//! the `update` tracing span call the time it spent.

use super::Msg;

impl Msg {
    /// The area's short name, e.g. "repo" for `Msg::Repo`. A scoped message is named after the
    /// message inside it.
    pub fn area(&self) -> &'static str {
        match self {
            Msg::Repo(_) => "repo",
            Msg::Recent(_) => "recent",
            Msg::Repos(_) => "repos",
            Msg::Load(_) => "load",
            Msg::Watch(_) => "watch",
            Msg::Selection(_) => "selection",
            Msg::Graph(_) => "graph",
            Msg::Search(_) => "search",
            Msg::Avatars(_) => "avatars",
            Msg::Diff(_) => "diff",
            Msg::Conflict(_) => "conflict",
            Msg::History(_) => "history",
            Msg::Blame(_) => "blame",
            Msg::Rebase(_) => "rebase",
            Msg::Stack(_) => "stack",
            Msg::Tabs(_) => "tabs",
            Msg::Stage(_) => "stage",
            Msg::Commit(_) => "commit",
            Msg::Job(_) => "job",
            Msg::Stash(_) => "stash",
            Msg::AutoStash(_) => "auto-stash",
            Msg::Files(_) => "files",
            Msg::WorkFile(_) => "work-file",
            Msg::Branch(_) => "branch",
            Msg::Clone(_) => "clone",
            Msg::Init(_) => "init",
            Msg::Tag(_) => "tag",
            Msg::Remote(_) => "remote",
            Msg::Worktree(_) => "worktree",
            Msg::Submodule(_) => "submodule",
            Msg::Terminal(_) => "terminal",
            Msg::Layout(_) => "layout",
            Msg::Patch(_) => "patch",
            Msg::Tools(_) => "tools",
            Msg::CustomCommand(_) => "custom-command",
            Msg::ConflictCheck(_) => "conflict-check",
            Msg::OpPreview(_) => "op-preview",
            Msg::Journal(_) => "journal",
            Msg::RangeDiff(_) => "range-diff",
            Msg::PullRequest(_) => "pull-request",
            Msg::Issue(_) => "issue",
            Msg::Forge(_) => "forge",
            Msg::ForgeFilter(_) => "forge-filter",
            Msg::Publish(_) => "publish",
            Msg::Credentials(_) => "credentials",
            Msg::Prefs(_) => "prefs",
            Msg::Keys(_) => "keys",
            Msg::Lfs(_) => "lfs",
            Msg::GitFlow(_) => "git-flow",
            Msg::Sparse(_) => "sparse",
            Msg::Ssh(_) => "ssh",
            Msg::Profile(_) => "profile",
            Msg::Palette(_) => "palette",
            Msg::Trust(_) => "trust",
            Msg::GitEnv(_) => "git-env",
            Msg::Theme(_) => "theme",
            Msg::Logs(_) => "logs",
            Msg::Onboarding(_) => "onboarding",
            Msg::DeepLink(_) => "deep-link",
            Msg::Dashboard(_) => "dashboard",
            Msg::Notify(_) => "notify",
            Msg::Dialog(_) => "dialog",
            Msg::App(_) => "app",
            Msg::Updates(_) => "updates",
            Msg::Partial(_) => "partial",
            Msg::Shortcut(_) => "shortcut",
            Msg::Tick(_) => "tick",
            Msg::Scoped { msg, .. } => msg.area(),
            Msg::Confirm { .. } | Msg::ConfirmDanger { .. } | Msg::ConfirmAccepted | Msg::ConfirmAlt | Msg::CloseModal => "confirm",
            Msg::ToastAction(_) | Msg::ToastDetails(_) => "toast",
            Msg::Copy(_) | Msg::OpenUrl(_) | Msg::OpenTerminal | Msg::OpenFolder(_) => "outside",
            Msg::ClearLog | Msg::Logged { .. } => "log",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::RepoMsg;
    use std::path::PathBuf;

    #[test]
    fn a_scoped_message_is_named_after_the_one_inside() {
        assert_eq!(Msg::Repo(RepoMsg::WindowFocused).area(), "repo");
        let scoped = Msg::Scoped { workdir: PathBuf::from("w"), msg: Box::new(Msg::Repo(RepoMsg::WindowFocused)) };
        assert_eq!(scoped.area(), "repo");
        assert_eq!(Msg::Tick(std::time::Instant::now()).area(), "tick");
    }
}
