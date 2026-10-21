//! The issue's description and comments, oldest first, as Markdown (the same way the pull
//! request conversation shows them; plain text from Azure Boards and Jira), or why they aren't
//! shown: loading, held while the repository isn't trusted, or the failure with Try again.

use super::super::markdown;
use super::super::markdown_cache::MarkdownCache;
use super::super::pane_status::{self, Failure};
use super::super::pull_request::note;
use super::super::theme::color;
use crate::msg::{IssueMsg, Msg};
use crate::state::AppState;
use crate::state::issue::{Details, IssueView};
use crate::state::pull_request::description_text;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::forge::Entry;
use glowfly_core::settings::Settings;
use glowfly_core::text::printable;

const LOADING: &str = "Loading the issue…";

pub(super) fn show(ui: &mut Ui, st: &AppState, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    let Some(v) = &st.issue else { return };
    egui::ScrollArea::vertical().id_salt("issue-conversation").auto_shrink([false, false]).show(ui, |ui| match &v.details {
        Details::Unsupported => {
            let text =
                format!("{} details aren't available in Glowfly for this tracker. Open it in the browser to see more.", capital(v.noun));
            note(ui, text, color::weak());
        }
        Details::Held => note(ui, "Trust this repository to load its issue.", color::weak()),
        Details::Loading => pane_status::loading(ui, LOADING),
        Details::Failed(e) => {
            let title = format!("Couldn't load this {}", v.noun);
            let retry = Some(Msg::Issue(IssueMsg::Reload));
            pane_status::failed(ui, st.settings.lang(), Failure { title: &title, error: e, retry }, msgs);
        }
        Details::Loaded(_) => conversation(ui, v, Clock { settings: &st.settings, now: st.view.now, offset: st.view.utc_offset }, md, msgs),
    });
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// What the comments' times are shown with.
#[derive(Clone, Copy)]
struct Clock<'a> {
    settings: &'a Settings,
    now: i64,
    offset: i32,
}

/// "<author> opened this issue", the description, then the comments.
fn conversation(ui: &mut Ui, v: &IssueView, clock: Clock<'_>, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    let Some(d) = v.loaded() else { return };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&d.author)).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        ui.label(RichText::new(format!("opened this {}", v.noun)).color(color::weak()));
    });
    let body = description_text(d.body.trim());
    if body.is_empty() {
        note(ui, "No description.", color::weak());
    } else {
        text(ui, v, "issue-description", &body, md, msgs);
    }
    ui.add_space(6.0);
    ui.separator();
    if d.comments.is_empty() {
        ui.add_space(4.0);
        note(ui, "No comments yet.", color::weak());
    }
    for (i, c) in d.comments.iter().enumerate() {
        comment(ui, v, i, c, clock, md, msgs);
    }
}

/// A description or comment: as Markdown from a tracker that writes it, else as plain text.
fn text(ui: &mut Ui, v: &IssueView, id: impl std::hash::Hash, body: &str, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    if v.markdown() {
        markdown::body(ui, id, body, md, msgs);
    } else {
        note(ui, body, color::text());
    }
}

fn comment(ui: &mut Ui, v: &IssueView, i: usize, c: &Entry, clock: Clock<'_>, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&c.author)).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        if c.time > 0 {
            crate::ui::date_tip::time_label(ui, clock.settings, c.time, clock.now, clock.offset);
        }
    });
    let body = description_text(c.body.trim());
    if body.is_empty() {
        note(ui, "(empty comment)", color::weak());
    } else {
        text(ui, v, ("issue-comment", i), &body, md, msgs);
    }
    ui.add_space(6.0);
    ui.separator();
}
