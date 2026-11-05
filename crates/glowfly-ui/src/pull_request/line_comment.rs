//! The new code comment's box, the first card of the Files changed pane under its file's diff:
//! where it goes (a link back to the lines), the lines it is on, the text, and Suggest a change
//! (quiet, leading), then at the trailing edge Cancel, Start a review and Add single comment
//! (filled, last; while a review is pending Add single comment, then Add review comment
//! filled), each disabled with its reason. Command+Enter posts with the main action; Esc leaves the field and cancels.

use super::super::theme::color;
use super::super::widgets::{self, Icon};
use super::{note, send_key};
use crate::theme::controls;
use crate::theme::tokens::radius;
use crate::widgets::buttons;
use eframe::egui::{self, Align, Key, Margin, RichText, Sense, Stroke, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::DiffLine;
use glowfly_core::text::printable;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::Msg;
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::pull_request::line_comment::LineCommentMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::pull_request::PrView;
use glowfly_model::state::pull_request::line_comment::{post_blocked, primary_is_review, review_label, suggest_blocked, where_text};

const WAITING: &str = "Waiting for the service…";

/// The least room between Suggest a change and the buttons at the trailing edge.
const LEAD_GAP: f32 = 16.0;

/// The lines of the snippet.
const SNIPPET_LINES: usize = 4;

/// Why each of the box's actions is unavailable, read before the panel is borrowed for the
/// text field.
pub(super) struct Reasons {
    single: Option<String>,
    review: Option<String>,
    suggest: Option<&'static str>,
}

pub(super) fn reasons(st: &AppState) -> Reasons {
    let provider = st.session.forge.as_ref().and_then(|f| f.host_provider());
    let Some(v) = &st.session.pull_request.view else { return Reasons { single: None, review: None, suggest: None } };
    Reasons {
        single: post_blocked(provider, v, false),
        review: post_blocked(provider, v, true),
        suggest: v.line_draft.as_ref().and_then(suggest_blocked),
    }
}

fn send(msgs: &mut Vec<Msg>, m: LineCommentMsg) {
    msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(m)));
}

/// A line of the snippet: its diff marker and indentation kept, the rest made safe to show.
fn snippet_line(l: &DiffLine) -> String {
    let marker = match l.origin {
        '+' | '-' => l.origin,
        _ => ' ',
    };
    let cut = l.text.find(|c: char| !matches!(c, ' ' | '\t')).unwrap_or(l.text.len());
    format!("{marker}{}{}", l.text[..cut].replace('\t', "    "), printable(&l.text[cut..]))
}

/// The draft's card; `focus` puts the cursor in its field and scrolls the pane to it.
/// `t` translates Cancel.
/// `text` is the frame's copy of its text (`PrFields::line_text`).
pub(super) fn card(
    ui: &mut Ui,
    t: Lang,
    (v, text): (&PrView, &mut Option<String>),
    r: &Reasons,
    keys: &Keymap,
    focus: bool,
    msgs: &mut Vec<Msg>,
) {
    let review = primary_is_review(v);
    let review_text = review_label(v);
    let (Some(d), Some(text)) = (v.line_draft.as_ref(), text.as_mut()) else { return };
    let posting = d.posting.is_some();
    let frame = egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(radius::SM)
        .inner_margin(Margin::symmetric(10, 6));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            // Painted: the bundled fonts have no speech-bubble glyph.
            widgets::paint_icon(ui.painter(), rect.center(), Icon::CommentAdd, color::weak());
            ui.label(RichText::new("New comment on").color(color::weak()));
            if ui.link(where_text(&d.anchor)).on_hover_text("Show these lines in the diff").clicked() {
                send(msgs, LineCommentMsg::Show);
            }
        });
        let skip = d.lines.len().saturating_sub(SNIPPET_LINES);
        let snippet = d.lines[skip..].iter().map(snippet_line).collect::<Vec<_>>().join("\n");
        ui.add(egui::Label::new(RichText::new(snippet).monospace().color(color::weak())).wrap());
        ui.add_space(4.0);
        let edit = egui::TextEdit::multiline(text)
            .interactive(!posting)
            .desired_rows(4)
            .hint_text("Leave a comment…")
            .desired_width(f32::INFINITY);
        let field = controls::field(ui, edit);
        crate::access::name_field(&field, "New comment");
        if focus {
            field.request_focus();
            field.scroll_to_me(Some(Align::Center));
        }
        let main_blocked = if review { &r.review } else { &r.single };
        if send_key(ui, &field) && main_blocked.is_none() {
            send(msgs, LineCommentMsg::Post { review });
        }
        if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            send(msgs, LineCommentMsg::Cancel);
        }
        let single_tip = keys.tip("Add single comment", BindingId::SendPrText);
        let review_tip = keys.tip(review_text, BindingId::SendPrText);
        let waiting = posting.then_some(WAITING);
        let suggest = buttons::quiet("Suggest a change").enabled(!posting && r.suggest.is_none()).why(waiting.or(r.suggest));
        let cancel = buttons::secondary(t.tr("common.cancel.button")).enabled(!posting).why(waiting);
        let style = |filled: bool, text: &'static str| if filled { buttons::primary(text) } else { buttons::secondary(text) };
        let single = style(!review, "Add single comment").enabled(r.single.is_none()).why(r.single.as_deref());
        let single = if review { single } else { single.tip(&single_tip) };
        let add = style(review, review_text).enabled(r.review.is_none()).why(r.review.as_deref());
        let add = if review { add.tip(&review_tip) } else { add };
        // The one filled button is last: Add single comment, or Add review comment while a review
        // is pending (Add single comment is then the other choice).
        let (main, other) = if review { ((add, true), (single, false)) } else { ((single, false), (add, true)) };
        let gap = ui.spacing().item_spacing.x;
        let widths = [&suggest, &cancel, &other.0, &main.0].map(|b| b.desired_width(ui));
        let fits = widths.iter().sum::<f32>() + 3.0 * gap + LEAD_GAP <= ui.available_width();
        let mut clicked = Vec::new();
        let mut add_button = |ui: &mut Ui, b: buttons::StyledButton<'_>, m: LineCommentMsg| {
            if ui.add(b).clicked() {
                clicked.push(m);
            }
        };
        let (main_msg, other_msg) = (LineCommentMsg::Post { review: main.1 }, LineCommentMsg::Post { review: other.1 });
        if fits {
            // Suggest a change leads; the others sit at the trailing edge. All in reading order,
            // so Tab follows it.
            let trailing = widths[1] + widths[2] + widths[3] + 2.0 * gap;
            ui.horizontal(|ui| {
                add_button(ui, suggest, LineCommentMsg::Suggest);
                ui.add_space((ui.available_width() - trailing).max(0.0));
                add_button(ui, cancel, LineCommentMsg::Cancel);
                add_button(ui, other.0, other_msg);
                add_button(ui, main.0, main_msg);
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                add_button(ui, suggest, LineCommentMsg::Suggest);
                add_button(ui, cancel, LineCommentMsg::Cancel);
                add_button(ui, other.0, other_msg);
                add_button(ui, main.0, main_msg);
            });
        }
        for m in clicked {
            send(msgs, m);
        }
        if let Some(e) = &d.error {
            note(ui, e.clone(), color::removed());
        }
    });
    ui.add_space(6.0);
}
