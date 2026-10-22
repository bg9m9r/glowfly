//! The issue panel's comment box, pinned under the conversation once the details have loaded:
//! the last post's failure, the draft (read-only while it is being posted, its height capped so
//! a long draft doesn't push the conversation away), a draft too long for the service, and the
//! Comment button with why it is disabled.

use crate::msg::{IssueMsg, Msg};
use crate::state::issue::IssueView;
use crate::state::issue_comment::{FIELD, comment_blocked, field_problem};
use crate::ui::access;
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::{buttons, reason};
use eframe::egui::{self, Align, Ui};

/// The tallest the draft gets before it scrolls.
const MAX_FIELD_H: f32 = 160.0;

/// Draws the box for panel `v`; `focus` puts the cursor at the end of the draft (after a Reply).
pub(super) fn show(ui: &mut Ui, v: &mut IssueView, trusted: bool, focus: bool, msgs: &mut Vec<Msg>) {
    if let Some(e) = &v.comment.error {
        form_message::form(ui, Message::error(format!("Couldn't post the comment: {e}")));
        ui.add_space(6.0);
    }
    let posting = v.comment.posting;
    let r = egui::ScrollArea::vertical().id_salt("issue-comment-field").max_height(MAX_FIELD_H).auto_shrink([false, true]).show(ui, |ui| {
        let edit = egui::TextEdit::multiline(&mut v.comment.text)
            .hint_text("Write a comment")
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .interactive(!posting);
        controls::field(ui, edit)
    });
    let r = r.inner;
    access::name_field(&r, "Comment");
    form_message::track(&mut v.comment.touched, FIELD, &r);
    if focus {
        r.request_focus();
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), r.id).unwrap_or_default();
        let end = egui::text::CCursor::new(v.comment.text.chars().count());
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
        state.store(ui.ctx(), r.id);
    }
    let problem = field_problem(v);
    let shown = problem.clone().filter(|_| v.comment.touched.shows(FIELD));
    form_message::field(ui, &v.comment.touched, FIELD, problem.map(Message::error));
    ui.add_space(4.0);
    let blocked = comment_blocked(v, trusted);
    let label = if posting { "Posting…" } else { "Comment" };
    if ui.add(buttons::secondary(label).enabled(blocked.is_none()).why(blocked.as_deref())).clicked() {
        msgs.push(Msg::Issue(IssueMsg::PostComment));
    }
    // The field's own problem already says it under the field: the same words don't show twice.
    if blocked.is_some() && blocked != shown {
        reason::line(ui, blocked.as_deref(), Align::Min);
    }
}
