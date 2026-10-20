//! Code comment threads in the pull request panel: the cards (in the conversation and in the
//! Files changed tab's pane under a file's diff), the Show resolved / Show outdated bar, and
//! the pane itself. Cards reply and resolve through `PullRequestMsg`s; their location links
//! open the thread's line in Files changed.

use super::super::markdown;
use super::super::markdown_cache::MarkdownCache;
use super::super::theme::color;
use super::super::widgets::{self, Icon};
use super::note;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{PrAction, PrView, description_text};
use crate::state::pull_request_actions::error_for;
use crate::state::pull_request_threads::{
    Placement, file_threads, go_to_open, hidden, hidden_text, is_open, kind_counts, on_file, outdated_toggle, placement, reply_blocked,
    resolve_blocked, resolve_label, resolved_toggle, where_text,
};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Color32, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};
use std::collections::HashMap;
use trellis_core::forge::ReviewThread;
use trellis_core::settings::Settings;
use trellis_core::text::printable;

/// Where a card is drawn: in the conversation (with its path and code snippet) or in its
/// file's pane.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Place {
    Overview,
    File,
}

/// What every card of a frame shares.
pub(super) struct Ctx<'a> {
    pub settings: &'a Settings,
    pub now: i64,
    pub offset: i32,
    /// Why going to each thread's line is unavailable (None when it is available).
    pub go_to: HashMap<String, Option<String>>,
    /// The thread to scroll to this frame.
    pub scroll: Option<String>,
}

/// Why going to each thread of the open panel's page is unavailable, computed before the
/// panel is borrowed for its text fields.
pub(super) fn go_to_reasons(st: &AppState) -> HashMap<String, Option<String>> {
    let Some(v) = &st.pull_request else { return HashMap::new() };
    let Some(p) = v.loaded() else { return HashMap::new() };
    p.threads.iter().map(|t| (t.id.clone(), go_to_open(st, v, t).err())).collect()
}

/// Show resolved / Show outdated (each only when there are such threads), and how many are
/// hidden.
pub(super) fn filter_bar(ui: &mut Ui, v: &PrView, path: Option<&str>, msgs: &mut Vec<Msg>) {
    let Some(p) = v.loaded() else { return };
    let (resolved, outdated) = kind_counts(p, path);
    if resolved + outdated == 0 {
        return;
    }
    let tv = &v.threads;
    ui.horizontal_wrapped(|ui| {
        if resolved > 0 && ui.add(buttons::secondary(resolved_toggle(tv, resolved))).clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::ShowResolved(!tv.show_resolved)));
        }
        if outdated > 0 && ui.add(buttons::secondary(outdated_toggle(tv, outdated))).clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::ShowOutdated(!tv.show_outdated)));
        }
        let (r, o) = hidden(p, tv, path);
        if let Some(text) = hidden_text(r, o) {
            ui.label(RichText::new(text).color(color::weak()));
        }
    });
    ui.add_space(4.0);
}

fn chip(ui: &mut Ui, text: &str, fg: Color32) {
    let w = ui.painter().layout_no_wrap(text.to_owned(), type_scale::font(type_scale::CAPTION), fg).size().x + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    widgets::chip(ui.painter(), rect.left(), rect.center().y, text, color::raised(), fg);
}

/// A link that goes to the thread's line, or says why it can't.
fn go_link(ui: &mut Ui, text: String, id: &str, blocked: Option<&String>, msgs: &mut Vec<Msg>) {
    let r = ui.add_enabled(blocked.is_none(), egui::Link::new(text));
    let r = match blocked {
        Some(why) => r.on_disabled_hover_text(why),
        None => r.on_hover_text("Show this line in Files changed"),
    };
    if r.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::GoToThread(id.to_owned())));
    }
}

/// Thread `id`'s card: where it is and its state, then (while open) the code it is on, its
/// comments, and the reply box with Reply and Resolve or Unresolve.
pub(super) fn card(ui: &mut Ui, v: &mut PrView, id: &str, place: Place, ctx: &Ctx, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    let Some(t) = v.loaded().and_then(|p| p.threads.iter().find(|t| t.id == id)).cloned() else { return };
    let open = is_open(&v.threads, &t);
    let focused = v.threads.focus.as_deref() == Some(id);
    let border = if focused { color::accent() } else { color::border() };
    let frame = egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(4)
        .inner_margin(Margin::symmetric(10, 6));
    let resp = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, &t, place, open, ctx, msgs);
            if open {
                body(ui, v, &t, place, ctx, md, msgs);
            }
        })
        .response;
    if ctx.scroll.as_deref() == Some(id) {
        resp.scroll_to_me(Some(Align::TOP));
    }
    let failed = error_for(v, &PrAction::Reply(t.id.clone())).or_else(|| error_for(v, &PrAction::Resolve(t.id.clone())));
    if let Some(e) = failed {
        note(ui, e.to_owned(), color::removed());
    }
    ui.add_space(6.0);
}

fn header(ui: &mut Ui, t: &ReviewThread, place: Place, open: bool, ctx: &Ctx, msgs: &mut Vec<Msg>) {
    let blocked = ctx.go_to.get(&t.id).cloned().flatten();
    // Show/Hide keeps its place on the right; the rest wraps in the width left of it.
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.add(buttons::secondary(if open { "Hide" } else { "Show" })).clicked() {
                msgs.push(Msg::PullRequest(PullRequestMsg::ToggleThread(t.id.clone())));
            }
            ui.with_layout(Layout::left_to_right(Align::Center).with_main_wrap(true), |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                // Painted: the bundled fonts have no speech-bubble glyph.
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Comment, color::weak());
                if place == Place::Overview {
                    go_link(ui, printable(&t.path), &t.id, blocked.as_ref(), msgs);
                }
                ui.label(RichText::new("on").color(color::weak()));
                go_link(ui, where_text(t), &t.id, blocked.as_ref(), msgs);
                if t.outdated {
                    chip(ui, "Outdated", color::warn());
                }
                if t.resolved {
                    chip(ui, "Resolved", color::added());
                    if let Some(by) = &t.resolved_by {
                        ui.label(RichText::new(format!("by {}", printable(by))).color(color::weak()));
                    }
                }
            });
        });
    });
}

fn body(ui: &mut Ui, v: &mut PrView, t: &ReviewThread, place: Place, ctx: &Ctx, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    if !t.context.is_empty() && (place == Place::Overview || t.outdated) {
        ui.add(egui::Label::new(RichText::new(&t.context).monospace().color(color::weak())).wrap());
        ui.add_space(2.0);
    }
    for (i, c) in t.comments.iter().enumerate() {
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(printable(&c.author)).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
            crate::ui::date_tip::time_label(ui, ctx.settings, c.time, ctx.now, ctx.offset);
            if c.pending {
                chip(ui, "Pending", color::weak());
            }
        });
        let text = description_text(c.body.trim());
        if !text.is_empty() {
            markdown::body(ui, ("pr-thread-comment", &t.id, i), &text, md, msgs);
        }
    }
    if t.more > 0 {
        ui.horizontal_wrapped(|ui| {
            let more =
                if t.more == 1 { "1 more comment on the service".to_owned() } else { format!("{} more comments on the service", t.more) };
            ui.label(RichText::new(more).color(color::weak()));
            let url = t.comments.first().map(|c| c.url.clone()).filter(|u| !u.is_empty());
            if let Some(url) = url
                && ui.link("Open in browser").clicked()
            {
                msgs.push(Msg::OpenUrl(url));
            }
        });
    }
    ui.add_space(4.0);
    let draft = v.forms.replies.entry(t.id.clone()).or_default();
    controls::field(ui, egui::TextEdit::multiline(draft).desired_rows(2).hint_text("Reply…").desired_width(f32::INFINITY));
    ui.horizontal(|ui| {
        let blocked = reply_blocked(v, t);
        let r = ui.add(buttons::secondary("Reply").enabled(blocked.is_none()));
        let r = match &blocked {
            Some(why) => r.on_hover_text(why),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::Reply(t.id.clone())));
        }
        let blocked = resolve_blocked(v, t);
        let r = ui.add(buttons::secondary(resolve_label(t)).enabled(blocked.is_none()));
        let r = match &blocked {
            Some(why) => r.on_hover_text(why),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::Resolve { thread: t.id.clone(), resolve: !t.resolved }));
        }
    });
}

/// The pane under a pull request file's diff: its threads, with the filter bar, and a note
/// when they can't be placed on the local lines.
pub(super) fn file_pane(ui: &mut Ui, st: &mut AppState, path: &str, md: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    let go_to = go_to_reasons(st);
    let scroll = st.view.pr_thread_scroll.take();
    let ctx = Ctx { settings: &st.settings, now: st.view.now, offset: st.view.utc_offset, go_to, scroll };
    let Some(v) = st.pull_request.as_mut() else { return };
    let Some(p) = v.loaded() else { return };
    let n = on_file(p, path);
    let ids: Vec<String> = file_threads(p, &v.threads, path).iter().map(|t| t.id.clone()).collect();
    ui.add_space(6.0);
    ui.label(type_scale::section_text(ui, format!("Comments on this file  {n}")));
    if let Placement::Elsewhere(why) = placement(v) {
        note(ui, why, color::weak());
    }
    filter_bar(ui, v, Some(path), msgs);
    egui::ScrollArea::vertical().id_salt("pr-file-threads").auto_shrink([false, false]).show(ui, |ui| {
        for id in &ids {
            card(ui, v, id, Place::File, &ctx, md, msgs);
        }
    });
}
