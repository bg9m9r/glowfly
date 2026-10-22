//! The pull request panel's Commits tab, on the left: the commits the source brings (the
//! target's tip..the source tip, oldest first), painted like file history's rows. A click shows
//! the commit in the commit panel; the commit's files open in the tab's center.

use super::super::common::{menu_item, menu_item_if, short};
use super::super::empty_state::{EmptyIcon, empty_state};
use super::super::history::NOT_ON_GRAPH;
use super::super::pane_status::{self, Failure};
use super::super::theme::color;
use super::super::widgets::paint_cut_text;
use super::files::notes;
use crate::msg::{Msg, RepoMsg};
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::{Files, PrView, unrelated_text};
use crate::state::pull_request_commits::{commits_heading, empty_text, more_text, second_line};
use crate::state::{AppState, Selection};
use crate::ui::theme::type_scale;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align2, CursorIcon, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::repo::RangeCommit;
use glowfly_core::text::printable;

const ROW_H: f32 = 44.0;
pub(in crate::ui) const LOADING: &str = "Reading the commits…";
pub(in crate::ui) const FAILED_TITLE: &str = "Couldn't read the commits";
const NO_COMMITS: &str = "No commits";
/// The tab's center while no commit diff is open.
pub(super) const HINT: &str = "Select a commit to see it in the commit panel; its files open here.";

pub(super) fn show(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let shown = v.shown.as_ref().filter(|_| matches!(v.files, Files::Loaded(_)));
    let heading = match shown {
        Some(s) => commits_heading(s.commits.commits.len() + s.commits.more),
        None => "Commits".to_owned(),
    };
    ui.label(type_scale::section_text(ui, heading));
    notes(ui, st, v, msgs);
    let weak = |ui: &mut Ui, text: String| ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
    match &v.files {
        Files::Missing => {}
        Files::Loading => pane_status::loading(ui, LOADING),
        Files::Unrelated => {
            let shallow = st.snap.as_ref().is_some_and(|s| !s.shallow_roots.is_empty());
            weak(ui, unrelated_text(v, shallow));
        }
        Files::Failed(e) => {
            let retry = Some(Msg::PullRequest(PullRequestMsg::ReloadFiles));
            pane_status::failed(ui, st.settings.lang(), Failure { title: FAILED_TITLE, error: e, retry }, msgs);
        }
        Files::Loaded(_) => {
            let Some(shown) = shown else { return };
            let commits = &shown.commits.commits;
            if commits.is_empty() {
                empty_state(ui, EmptyIcon::Commits, NO_COMMITS, &empty_text(v), &[]);
                return;
            }
            if let Some(more) = more_text(shown) {
                weak(ui, more);
            }
            let selected = match st.selection {
                Selection::Commit(o) => Some(o),
                _ => None,
            };
            egui::ScrollArea::vertical().id_salt("pr-commits").auto_shrink([false, false]).show_rows(
                ui,
                ROW_H,
                commits.len(),
                |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for c in &commits[range] {
                        row(ui, st, c, selected == Some(c.oid), msgs);
                    }
                },
            );
        }
    }
}

/// One commit: the short SHA and summary, then the author (and "merge") with the day on the
/// right (the full date on hover). Clicking selects it; its menu copies the SHA or shows it on
/// the graph.
fn row(ui: &mut Ui, st: &AppState, c: &RangeCommit, selected: bool, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    crate::ui::access::name_button(&resp, true, &format!("{} {}", short(c.oid), printable(&c.summary)));
    let date_rect = ui.is_rect_visible(rect).then(|| {
        super::super::widgets::row_fill::paint(ui.painter(), rect, selected, resp.hovered());
        paint(ui, st, rect, c)
    });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let resp = match (date_rect, ui.ctx().pointer_hover_pos()) {
        (Some(r), Some(p)) if resp.hovered() && r.contains(p) => {
            resp.on_hover_text_at_pointer(st.settings.date_tooltip(c.time, st.view.now, st.view.utc_offset))
        }
        _ => resp,
    };
    if resp.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::SelectCommit(c.oid)));
    }
    let on_graph = st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&c.oid));
    resp.row_menu(|ui| {
        menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(c.oid.to_string()));
        menu_item_if(msgs, ui, "Show on graph", (!on_graph).then_some(NOT_ON_GRAPH), Msg::Repo(RepoMsg::SelectCommit(c.oid)));
    });
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
}

/// Paints a row's two lines; returns where the day was painted.
fn paint(ui: &Ui, st: &AppState, rect: egui::Rect, c: &RangeCommit) -> egui::Rect {
    let mono = type_scale::mono(type_scale::SMALL);
    let body = type_scale::font(type_scale::BODY);
    let small = type_scale::font(type_scale::SMALL);
    let (left, right) = (rect.left() + 12.0, rect.right() - 10.0);
    let (y1, y2) = (rect.top() + 13.0, rect.top() + 31.0);
    let p = ui.painter();
    let sha_w = paint_cut_text(p, pos2(left, y1), &short(c.oid), mono, color::weak(), 80.0).rect.width();
    paint_cut_text(p, pos2(left + sha_w + 8.0, y1), &printable(&c.summary), body, color::text(), right - left - sha_w - 8.0);
    let date = st.settings.format_day(c.time, st.view.now, st.view.utc_offset);
    let date_rect = p.text(pos2(right, y2), Align2::RIGHT_CENTER, date, small.clone(), color::weak());
    let limit = right - date_rect.width() - 12.0;
    paint_cut_text(p, pos2(left, y2), &second_line(c), small, color::text(), limit - left);
    date_rect
}
