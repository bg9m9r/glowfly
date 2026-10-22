//! Messages of a new code comment on lines of a pull request file's diff (the comment button on
//! a diff line, the hunk header's Comment on N lines, and the comment box in the Files changed
//! pane). Handled by update/pull_request/line_comment.rs; its rules are in
//! state/pull_request_line_comment.rs.

use glowfly_core::diff_rows::LineSide;

#[derive(Clone, Debug, PartialEq)]
pub enum LineCommentMsg {
    /// A diff line's comment button: start a draft on file line `line` of `side` (or on the
    /// selection it is part of); with `extend` (Shift), stretch the open draft to it instead.
    Start { side: LineSide, line: u32, extend: bool },
    /// The hunk header's Comment on N lines: start a draft on the selected lines' span.
    OnSelection,
    /// Suggest a change: add a suggestion block of the lines to the draft.
    Suggest,
    /// Post the draft: alone (Add single comment) or in the pending review (`review`: Start a
    /// review, Add review comment).
    Post { review: bool },
    /// Cancel, or Esc: drop an empty draft, or ask before dropping an edited one.
    Cancel,
    /// Drop the draft (after the confirmation).
    Discard,
    /// The draft's marker in the diff: focus the comment box.
    Focus,
    /// The comment box's line link: scroll the diff to the draft's lines.
    Show,
}
