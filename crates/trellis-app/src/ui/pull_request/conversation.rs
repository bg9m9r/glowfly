//! The pull request's conversation: who opened it and its description, then every comment and
//! submitted review, oldest first, with the comment box pinned below. Services without
//! in-app reviews show a note instead of the timeline and the box.

use super::super::theme::color;
use super::{note, tone_color};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction, PrView, description_text};
use crate::state::pull_request_actions::{comment_blocked, error_for, verdict_text};
use eframe::egui::{self, Margin, RichText, Ui};
use trellis_core::forge::{Entry, EntryKind, Verdict};
use trellis_core::settings::Settings;
use trellis_core::text::printable;

pub(super) fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let (settings, now, offset) = (&st.settings, st.view.now, st.view.utc_offset);
    let Some(v) = st.pull_request.as_mut() else { return };
    let supported = !matches!(v.page, Page::Unsupported);
    if supported {
        egui::TopBottomPanel::bottom("pr-comment-box")
            .frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(16, 8)))
            .show_inside(ui, |ui| comment_box(ui, v, msgs));
    }
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(16, 10))).show_inside(
        ui,
        |ui| {
            egui::ScrollArea::vertical().id_salt("pr-conversation").auto_shrink([false, false]).show(ui, |ui| {
                opening(ui, v);
                match &v.page {
                    Page::Unsupported => {
                        let text = format!(
                            "Comments, reviews and merging in Trellis work with GitHub for now. Open this {} in the browser to take part.",
                            v.noun
                        );
                        note(ui, text, color::weak());
                    }
                    Page::Held => note(ui, "Trust this repository to load its pull request.", color::weak()),
                    Page::Loading => note(ui, "Loading the conversation…", color::weak()),
                    Page::Failed(e) => {
                        note(ui, e.as_str(), color::red());
                        if ui.button("Try again").clicked() {
                            msgs.push(Msg::PullRequest(PullRequestMsg::Reload));
                        }
                    }
                    Page::Loaded(p) => {
                        for e in &p.entries {
                            entry(ui, e, settings, now, offset);
                        }
                    }
                }
            });
        },
    );
}

/// "<author> opened this pull request", and its description.
fn opening(ui: &mut Ui, v: &PrView) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&v.pr.author)).strong().color(color::text()));
        ui.label(RichText::new(format!("opened this {}", v.noun)).color(color::weak()));
    });
    let d = description_text(v.pr.description.trim());
    if d.is_empty() {
        note(ui, "No description.", color::weak());
    } else {
        note(ui, d, color::text());
    }
    ui.add_space(6.0);
    ui.separator();
}

/// One comment or review: who, what they did and when, then what they wrote.
fn entry(ui: &mut Ui, e: &Entry, settings: &Settings, now: i64, offset: i32) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&e.author)).strong().color(color::text()));
        let (did, tone) = verdict_text(&e.kind);
        ui.label(RichText::new(did).color(tone_color(tone)));
        ui.label(RichText::new(settings.format_time(e.time, now, offset)).color(color::weak()));
    });
    let body = description_text(e.body.trim());
    if !body.is_empty() {
        note(ui, body, color::text());
    } else if e.kind == EntryKind::Review(Verdict::Commented) {
        note(ui, "Left comments on the code.", color::weak());
    }
    ui.add_space(6.0);
    ui.separator();
}

/// The comment box and its button, with the last post's failure above it.
fn comment_box(ui: &mut Ui, v: &mut PrView, msgs: &mut Vec<Msg>) {
    if let Some(e) = error_for(v, PrAction::Comment) {
        note(ui, e.to_owned(), color::red());
    }
    let edit = egui::TextEdit::multiline(&mut v.forms.comment).hint_text("Write a comment").desired_rows(3).desired_width(f32::INFINITY);
    ui.add(edit);
    let blocked = comment_blocked(v);
    ui.horizontal(|ui| {
        let mut r = ui.add_enabled(blocked.is_none(), egui::Button::new("Comment"));
        if let Some(why) = &blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::PostComment));
        }
    });
}
