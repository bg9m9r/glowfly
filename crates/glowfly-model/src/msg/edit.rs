//! What the views edited this frame. A view draws a text field or checkbox whose value update
//! also reads (the commit draft, the welcome's form, a terminal tab's rename field and mouse
//! selection, the blame and File view's find and Line fields, the settings, the open dialog's
//! form, Preferences' own forms, the palette's query, the pull request and issue drafts, the
//! rebase editor and the conflict output) from a copy taken for the frame, and when the copy
//! changed it pushes the copy back as one `Msg::Edit`
//! (update/edit.rs), naming what it was drawn from (the tab, file version, session or blame)
//! so it never lands on anything else. The shell moves every edit ahead of the frame's other messages
//! (`edits_first`), so a key handled in the same frame (Ctrl+Enter commits, Enter renames)
//! sees the keystrokes typed with it, and asks for one more frame so what was drawn before the
//! field catches up. Secrets (tokens, passphrases) never travel here: they stay in
//! `ViewState::secrets`.

use super::{Inactive, Msg};
use crate::state::blame::BlameFields;
use crate::state::commit::CommitFields;
use crate::state::history_text::TextNav;
use crate::state::issue_comment::CommentFields;
use crate::state::modal_form::DialogForm;
use crate::state::onboarding::WelcomeFields;
use crate::state::prefs_fields::PrefsFields;
use crate::state::pull_request::actions::PrFields;
use crate::state::rebase::Editing;
use crate::state::ssh::KeygenFields;
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
    /// The open dialog's edited form (kept only while a dialog of the same kind is open).
    Dialog(DialogForm),
    /// The danger confirmation's acknowledgement box.
    ConfirmAck(bool),
    /// The auto-stash question's "Always stash" box.
    AutoStashAlways(bool),
    /// Generate SSH key's Location, Comment and touched state (kept only while Preferences is open).
    SshKeygen(KeygenFields),
    /// Preferences' own forms (kept only while Preferences is open).
    Prefs(Box<PrefsFields>),
    /// The palette's query, typed in the step `generation` (`AppState::palette_gen`).
    PaletteQuery { generation: u64, query: String },
    /// The pull request panel's forms and new code comment's text, for the pull request `id`.
    PullRequest(Box<PrFields>),
    /// The issue panel's comment box, for the issue `key`.
    IssueComment { key: String, fields: CommentFields },
    /// The interactive rebase panel's message or command editor, open on `editing`.
    RebaseDraft { editing: Editing, draft: String },
    /// The conflict output's hand-edited text, for the conflict file `path`.
    ConflictOutput { path: String, buffer: String },
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
            | Self::Settings(_)
            | Self::Dialog(_)
            | Self::ConfirmAck(_)
            | Self::AutoStashAlways(_)
            | Self::SshKeygen(_)
            | Self::Prefs(_)
            | Self::PaletteQuery { .. }
            | Self::PullRequest(_)
            | Self::IssueComment { .. }
            | Self::RebaseDraft { .. }
            | Self::ConflictOutput { .. } => None,
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
