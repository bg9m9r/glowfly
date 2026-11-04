//! The "Your review" section: the verdict (comment, approve, request changes; on GitLab,
//! comment, approve or remove approval; on Bitbucket, comment, approve or request changes,
//! either taken back once given), a summary, and the Submit review button, or Finish review for
//! a review already started on the service, with why it is disabled under it. A choice that is
//! unavailable says why in place of its help line.

use super::super::theme::color;
use super::{heading, note, send_key};
use crate::theme::controls;
use crate::widgets::{buttons, reason};
use eframe::egui::{self, Ui};
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::Msg;
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::pull_request::{PrAction, PrView};
use glowfly_model::state::pull_request_actions::{
    error_for, review_blocked, review_button, review_choice_blocked, review_choice_line, review_choice_text, review_choices,
    review_placeholder,
};

/// Shown once the page is loaded. Command+Enter in the summary submits too.
pub(super) fn show(ui: &mut Ui, keys: &Keymap, v: &mut PrView, msgs: &mut Vec<Msg>) {
    let Some(page) = v.loaded() else { return };
    let pending = page.detail.pending_review.is_some();
    let detail = page.detail.clone();
    heading(ui, "Your review");
    for ev in review_choices(&detail) {
        let label = review_choice_text(ev).0;
        let blocked = review_choice_blocked(v, ev);
        let checked = v.forms.review_event == ev;
        if ui.add_enabled(blocked.is_none(), controls::Radio::new(checked, label)).clicked() {
            v.forms.review_event = ev;
        }
        // The help, or why this choice is unavailable, starts where the label does (the space
        // that opens a row takes no item spacing after it).
        let s = ui.spacing();
        let indent = s.icon_width + s.icon_spacing;
        let line = review_choice_line(v, ev);
        ui.horizontal(|ui| {
            ui.add_space(indent);
            note(ui, line, color::weak());
        });
        ui.add_space(4.0);
    }
    let hint = review_placeholder(v);
    let summary = egui::TextEdit::multiline(&mut v.forms.review_body).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY);
    let field = controls::field(ui, summary);
    crate::access::name_field(&field, "Review summary");
    if send_key(ui, &field) && review_blocked(v).is_none() {
        msgs.push(Msg::PullRequest(PullRequestMsg::SubmitReview));
    }
    if pending {
        note(ui, "You have a review in progress on the service; this submits it with its comments.", color::weak());
    }
    let blocked = review_blocked(v);
    let label = review_button(v);
    let tip = keys.tip(label, BindingId::SendPrText);
    let r = ui.add(buttons::secondary(label).enabled(blocked.is_none()).why(blocked.as_deref()).tip(&tip));
    if blocked.is_some() {
        reason::line(ui, blocked.as_deref(), egui::Align::Min);
    }
    if r.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::SubmitReview));
    }
    if let Some(e) = error_for(v, &PrAction::Review) {
        note(ui, e.to_owned(), color::removed());
    }
}
