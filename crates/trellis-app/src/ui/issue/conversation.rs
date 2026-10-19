//! The issue's description and comments, oldest first, as plain wrapped text (the same way the
//! pull request conversation shows them), or why they aren't shown: loading, held while the
//! repository isn't trusted, or the failure with Try again.

use super::super::pane_status::{self, Failure};
use super::super::pull_request::note;
use super::super::theme::color;
use crate::msg::{IssueMsg, Msg};
use crate::state::AppState;
use crate::state::issue::{Details, IssueView};
use crate::state::pull_request::description_text;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use trellis_core::forge::Entry;
use trellis_core::settings::Settings;
use trellis_core::text::printable;

const LOADING: &str = "Loading the issue…";

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(v) = &st.issue else { return };
    egui::ScrollArea::vertical().id_salt("issue-conversation").auto_shrink([false, false]).show(ui, |ui| match &v.details {
        Details::Unsupported => {
            let text =
                format!("{} details aren't available in Trellis for this tracker. Open it in the browser to see more.", capital(v.noun));
            note(ui, text, color::weak());
        }
        Details::Held => note(ui, "Trust this repository to load its issue.", color::weak()),
        Details::Loading => pane_status::loading(ui, LOADING),
        Details::Failed(e) => {
            let title = format!("Couldn't load this {}", v.noun);
            let retry = Some(Msg::Issue(IssueMsg::Reload));
            pane_status::failed(ui, st.settings.lang(), Failure { title: &title, error: e, retry }, msgs);
        }
        Details::Loaded(_) => conversation(ui, v, &st.settings, st.view.now, st.view.utc_offset),
    });
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// "<author> opened this issue", the description, then the comments.
fn conversation(ui: &mut Ui, v: &IssueView, settings: &Settings, now: i64, offset: i32) {
    let Some(d) = v.loaded() else { return };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&d.author)).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        ui.label(RichText::new(format!("opened this {}", v.noun)).color(color::weak()));
    });
    let body = description_text(d.body.trim());
    if body.is_empty() {
        note(ui, "No description.", color::weak());
    } else {
        note(ui, body, color::text());
    }
    ui.add_space(6.0);
    ui.separator();
    if d.comments.is_empty() {
        ui.add_space(4.0);
        note(ui, "No comments yet.", color::weak());
    }
    for c in &d.comments {
        comment(ui, c, settings, now, offset);
    }
}

fn comment(ui: &mut Ui, c: &Entry, settings: &Settings, now: i64, offset: i32) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(printable(&c.author)).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        if c.time > 0 {
            crate::ui::date_tip::time_label(ui, settings, c.time, now, offset);
        }
    });
    let body = description_text(c.body.trim());
    if body.is_empty() {
        note(ui, "(empty comment)", color::weak());
    } else {
        note(ui, body, color::text());
    }
    ui.add_space(6.0);
    ui.separator();
}
