//! What the views edited this frame. A view draws a text field or checkbox whose value update
//! also reads (the commit draft, the welcome's form, a terminal tab's rename field and mouse
//! selection, the blame and File view's find and Line fields, the settings) from a copy taken
//! for the frame, and when the copy changed it pushes the copy back as one `Msg::Edit`
//! (update/edit.rs), naming what it was drawn from (the tab, file version, session or blame)
//! so it never lands on anything else. The shell moves every edit ahead of the frame's other messages
//! (`edits_first`), so a key handled in the same frame (Ctrl+Enter commits, Enter renames)
//! sees the keystrokes typed with it, and asks for one more frame so what was drawn before the
//! field catches up.

use super::{Inactive, Msg};
use crate::state::blame::BlameFields;
use crate::state::commit::CommitFields;
use crate::state::history_text::TextNav;
use crate::state::onboarding::WelcomeFields;
use crate::state::tabs::TabId;
use crate::state::terminal::{Selection, TabRename};
use git2::Oid;
use glowfly_core::settings::Settings;

/// A view's edited copy of the model fields it draws (update/edit.rs).
#[derive(Debug)]
pub enum EditMsg {
    /// The commit box's summary, description, Amend and Skip hooks, drawn in the tab `tab`
    /// (kept only while that tab is active, as the draft is parked with it).
    Commit { tab: TabId, fields: CommitFields },
    /// The welcome's name, email, tour and avatars choices.
    Welcome(WelcomeFields),
    /// A terminal tab's rename field (kept only while that tab's rename is open).
    TerminalRename(TabRename),
    /// The mouse selection in the terminal session `id`.
    TerminalSelection { id: u64, selection: Option<Selection> },
    /// The blame view's find and Line fields, top and cursor lines and taken requests.
    Blame(Box<BlameFields>),
    /// The File view's find and Line fields, cursor and scroll, drawn for the version `oid` of
    /// `path` (kept only while the File view still shows that version).
    FileNav { oid: Oid, path: String, nav: Box<TextNav> },
    /// The settings as Preferences' pages left them.
    Settings(Box<Settings>),
}

impl EditMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as an edit is a user's and never comes back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Commit { .. }
            | Self::Welcome(_)
            | Self::TerminalRename(_)
            | Self::TerminalSelection { .. }
            | Self::Blame(_)
            | Self::FileNav { .. }
            | Self::Settings(_) => None,
        }
    }
}

impl Msg {
    /// Whether this is a view's edit, which goes ahead of the frame's other messages.
    pub fn is_edit(&self) -> bool {
        matches!(self, Msg::Edit(_))
    }
}

/// Moves the views' edits ahead of the frame's other messages, keeping the order within both;
/// whether there were any (the shell then draws one more frame).
pub fn edits_first(msgs: &mut Vec<Msg>) -> bool {
    let (edits, rest): (Vec<Msg>, Vec<Msg>) = std::mem::take(msgs).into_iter().partition(Msg::is_edit);
    let any = !edits.is_empty();
    *msgs = edits;
    msgs.extend(rest);
    any
}
