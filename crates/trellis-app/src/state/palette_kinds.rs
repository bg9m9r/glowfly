//! The palette's second steps: the sub-lists a command opens (tracked files, stashes) and
//! free-text steps (a stash message, a commit count), with their titles, prompts and footers.

use super::palette_commands::PaletteCommand;
use trellis_core::clone_ops::parse_depth;

/// What a stash sub-list does with the chosen stash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashAction {
    Apply,
    Pop,
    Drop,
}

impl StashAction {
    /// The `git stash` subcommand.
    pub fn git_verb(self) -> &'static str {
        match self {
            StashAction::Apply => "apply",
            StashAction::Pop => "pop",
            StashAction::Drop => "drop",
        }
    }

    /// The job's label in the activity log, as the sidebar names it.
    pub fn job_label(self) -> &'static str {
        match self {
            StashAction::Apply => "Apply stash",
            StashAction::Pop => "Pop stash",
            StashAction::Drop => "Drop stash",
        }
    }
}

/// A second step: the palette lists items for one command, with its own prompt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickKind {
    FileHistory,
    Stash(StashAction),
    /// Choose a stash to open the Rename stash dialog for.
    RenameStash,
}

impl PickKind {
    /// The step label beside the search field.
    pub fn title(self) -> &'static str {
        match self {
            PickKind::FileHistory => "File history",
            PickKind::Stash(StashAction::Apply) => "Apply stash",
            PickKind::Stash(StashAction::Pop) => "Pop stash",
            PickKind::Stash(StashAction::Drop) => "Delete stash",
            PickKind::RenameStash => "Rename stash",
        }
    }

    /// The search field's hint.
    pub fn prompt(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Find a file to show its history",
            PickKind::Stash(StashAction::Apply) => "Find a stash to apply",
            PickKind::Stash(StashAction::Pop) => "Find a stash to pop",
            PickKind::Stash(StashAction::Drop) => "Find a stash to delete",
            PickKind::RenameStash => "Find a stash to rename",
        }
    }

    pub fn loading(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Loading files…",
            PickKind::Stash(_) | PickKind::RenameStash => "Loading stashes…",
        }
    }

    /// Shown when nothing matches the query.
    pub fn none_text(self) -> &'static str {
        match self {
            PickKind::FileHistory => "No matching files",
            PickKind::Stash(_) | PickKind::RenameStash => "No matching stashes",
        }
    }

    /// Shown when there is nothing to pick at all.
    pub fn empty_text(self) -> &'static str {
        match self {
            PickKind::FileHistory => "This repository has no tracked files",
            PickKind::Stash(_) | PickKind::RenameStash => "This repository has no stashes",
        }
    }

    pub fn footer(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Up/Down to choose, Enter to show history, Backspace or Esc to go back",
            PickKind::Stash(a) => match a {
                StashAction::Apply => "Up/Down to choose, Enter to apply, Backspace or Esc to go back",
                StashAction::Pop => "Up/Down to choose, Enter to pop, Backspace or Esc to go back",
                StashAction::Drop => "Up/Down to choose, Enter to delete, Backspace or Esc to go back",
            },
            PickKind::RenameStash => "Up/Down to choose, Enter to rename, Backspace or Esc to go back",
        }
    }

    /// The line under a list cut short at `shown` of `total` matches.
    pub fn more_text(self, shown: usize, total: usize) -> String {
        let noun = match self {
            PickKind::FileHistory => "files",
            PickKind::Stash(_) | PickKind::RenameStash => "stashes",
        };
        format!("Showing {shown} of {total} {noun}. Type more to narrow the list.")
    }

    /// Whether rows dim their folder part (through the last '/').
    pub fn dims_folders(self) -> bool {
        matches!(self, PickKind::FileHistory)
    }
}

/// A free-text step: the palette's field takes text for one command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputKind {
    StashMessage,
    /// How many more commits of a shallow clone to download.
    DeepenBy,
}

impl InputKind {
    /// The step label beside the field.
    pub fn title(self) -> &'static str {
        match self {
            InputKind::StashMessage => "Stash with message",
            InputKind::DeepenBy => "Fetch more commits",
        }
    }

    /// The field's hint.
    pub fn prompt(self) -> &'static str {
        match self {
            InputKind::StashMessage => "Message for the new stash",
            InputKind::DeepenBy => "How many more commits to download",
        }
    }

    /// The dim line under the field.
    pub fn note(self) -> &'static str {
        match self {
            InputKind::StashMessage => "Stashes all changes, untracked files included.",
            InputKind::DeepenBy => "Downloads this many older commits below where each fetched branch's history now stops.",
        }
    }

    pub fn footer(self) -> &'static str {
        match self {
            InputKind::StashMessage => "Enter to stash, Backspace or Esc to go back",
            InputKind::DeepenBy => "Enter to fetch, Backspace or Esc to go back",
        }
    }

    /// What Enter runs for `text`: nothing while it is blank.
    pub fn command(self, text: &str) -> Option<PaletteCommand> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        match self {
            InputKind::StashMessage => Some(PaletteCommand::StashWithMessage(t.to_owned())),
            InputKind::DeepenBy => parse_depth(t).ok().map(PaletteCommand::FetchMoreCommitsBy),
        }
    }

    /// Why Enter does nothing for `text`, shown in red in place of the note; None while the
    /// text is blank or usable.
    pub fn problem(self, text: &str) -> Option<&'static str> {
        match self {
            InputKind::StashMessage => None,
            InputKind::DeepenBy if text.trim().is_empty() => None,
            InputKind::DeepenBy => parse_depth(text).err().map(|p| p.message()),
        }
    }
}
