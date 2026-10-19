//! The "Your review" section: the verdict (comment, approve, request changes), a summary, and
//! the Submit review button, or Finish review for a review already started on the service.

use super::super::theme::color;
use super::{heading, note};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::{PrAction, PrView};
use crate::state::pull_request_actions::{error_for, review_blocked, review_button, review_choice_blocked};
use crate::ui::theme::controls;
use eframe::egui::{self, Ui};
use trellis_core::forge::ReviewEvent;

const CHOICES: [(ReviewEvent, &str, &str); 3] = [
    (ReviewEvent::Comment, "Comment", "General feedback, without approving."),
    (ReviewEvent::Approve, "Approve", "Approve these changes for merging."),
    (ReviewEvent::RequestChanges, "Request changes", "Feedback that must be addressed before this can merge."),
];

/// Shown once the page is loaded.
pub(super) fn show(ui: &mut Ui, v: &mut PrView, msgs: &mut Vec<Msg>) {
    let Some(page) = v.loaded() else { return };
    let pending = page.detail.pending_review.is_some();
    heading(ui, "Your review");
    for (ev, label, help) in CHOICES {
        let blocked = review_choice_blocked(v, ev);
        let checked = v.forms.review_event == ev;
        let mut r = ui.add_enabled(blocked.is_none(), controls::Radio::new(checked, label));
        if let Some(why) = &blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() {
            v.forms.review_event = ev;
        }
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            note(ui, help, color::weak());
        });
    }
    ui.add_space(4.0);
    let summary = egui::TextEdit::multiline(&mut v.forms.review_body)
        .hint_text("Summary (optional when approving)")
        .desired_rows(3)
        .desired_width(f32::INFINITY);
    controls::field(ui, summary);
    if pending {
        note(ui, "You have a review in progress on the service; this submits it with its comments.", color::weak());
    }
    let blocked = review_blocked(v);
    let mut r = ui.add_enabled(blocked.is_none(), egui::Button::new(review_button(v)));
    if let Some(why) = &blocked {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::SubmitReview));
    }
    if let Some(e) = error_for(v, &PrAction::Review) {
        note(ui, e.to_owned(), color::red());
    }
}
