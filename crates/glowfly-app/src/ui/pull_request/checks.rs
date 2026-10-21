//! The pull request panel's Checks section, in the Overview side panel: a header that opens and
//! closes it with the overall marker and summary, then the build page link and one row per
//! check (its painted state, group and name, a Required chip and how long it took), or the
//! section's loading, failed, held or empty state.

use super::super::build_status::{paint_marker, paint_run};
use super::super::pane_status::{self, Failure};
use super::super::theme::color;
use super::super::widgets::{self, paint_cut, paint_cut_text};
use super::note;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::PrView;
use crate::state::pull_request_checks::{
    ChecksPane, FAILED_TITLE, HELD, LOADING, NO_STATUS, build_link, empty_text, header_summary, run_tooltip, time_text, truncated_text,
};
use crate::ui::theme::type_scale;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, CursorIcon, Rect, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::forge::{CheckList, CheckRun};
use glowfly_core::i18n::Lang;
use glowfly_core::text::printable;

/// The header row's height, and each check row's.
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 26.0;
/// Where a row's icon sits and its text starts, from the row's left edge.
const ICON_X: f32 = 10.0;
const TEXT_X: f32 = 22.0;
/// The gap between a row's parts.
const GAP: f32 = 8.0;
const REQUIRED: &str = "Required";

pub(super) fn show(ui: &mut Ui, v: &PrView, t: Lang, now: i64, msgs: &mut Vec<Msg>) {
    if v.checks.pane == ChecksPane::Unsupported {
        return;
    }
    ui.add_space(10.0);
    ui.separator();
    header(ui, v, msgs);
    if !v.checks.open {
        return;
    }
    match &v.checks.pane {
        ChecksPane::Unsupported => {}
        ChecksPane::Held => note(ui, HELD, color::weak()),
        ChecksPane::Loading => pane_status::loading_line(ui, LOADING),
        ChecksPane::Failed(e) => {
            let retry = Some(Msg::PullRequest(PullRequestMsg::ReloadChecks));
            pane_status::failed_line(ui, t, Failure { title: FAILED_TITLE, error: e, retry }, msgs);
        }
        ChecksPane::Loaded(list) if list.runs.is_empty() => {
            ui.add_space(2.0);
            ui.label(RichText::new(NO_STATUS).color(color::text()));
            let caption = RichText::new(empty_text(v.noun)).size(type_scale::CAPTION).color(color::weak());
            ui.add(egui::Label::new(caption).wrap());
        }
        ChecksPane::Loaded(list) => loaded(ui, v, list, t, now, msgs),
    }
}

/// The chevron, "Checks", the overall marker and summary, and the count; a click opens or
/// closes the section.
fn header(ui: &mut Ui, v: &PrView, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Checks"));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let y = rect.center().y;
    widgets::chevron(p, pos2(rect.left() + 8.0, y), v.checks.open, color::weak());
    let title = p.text(pos2(rect.left() + 20.0, y), Align2::LEFT_CENTER, "Checks", type_scale::section(ui), color::text());
    let body = type_scale::font(type_scale::BODY);
    let mut right = rect.right() - 4.0;
    if let Some(n) = v.checks.loaded().map(|l| l.runs.len()).filter(|&n| n > 0) {
        let count = p.text(pos2(right, y), Align2::RIGHT_CENTER, n.to_string(), body.clone(), color::weak());
        right = count.left() - GAP;
    }
    let (state, summary) = header_summary(v);
    let mut x = title.right() + 12.0;
    if let Some(s) = state {
        paint_marker(p, pos2(x + 5.0, y), s);
        x += 10.0 + GAP;
    }
    if !summary.is_empty() {
        paint_cut_text(p, pos2(x, y), &summary, body, color::weak(), right - x);
    }
    if resp.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::ToggleChecks));
    }
}

/// The build page link, each check, and the note when the list was cut.
fn loaded(ui: &mut Ui, v: &PrView, list: &CheckList, t: Lang, now: i64, msgs: &mut Vec<Msg>) {
    if let Some((label, url)) = build_link(v)
        && ui.link(label).clicked()
    {
        msgs.push(Msg::OpenUrl(url));
    }
    ui.add_space(2.0);
    for r in &list.runs {
        row(ui, r, t, now, msgs);
    }
    if !list.complete {
        ui.add_space(4.0);
        note(ui, truncated_text(), color::weak());
        if !v.pr.url.is_empty() && ui.link("See all on the service").clicked() {
            msgs.push(Msg::OpenUrl(v.pr.url.clone()));
        }
    }
}

/// The group in weak text, then the name, on one line.
fn label_job(r: &CheckRun) -> LayoutJob {
    let font = type_scale::font(type_scale::BODY);
    let mut job = LayoutJob::default();
    if let Some(g) = r.group.as_deref().map(printable).filter(|g| !g.is_empty()) {
        job.append(&format!("{g} · "), 0.0, TextFormat::simple(font.clone(), color::weak()));
    }
    job.append(&printable(&r.name), 0.0, TextFormat::simple(font, color::text()));
    job
}

/// One check: its icon, group and name, a Required chip and its time; a click opens its page.
fn row(ui: &mut Ui, r: &CheckRun, t: Lang, now: i64, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let name = format!("{}: {}", printable(&r.name), r.state.word());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    if ui.is_rect_visible(rect) {
        paint_row(ui, rect, r, now, resp.hovered());
    }
    let linked = !r.url.is_empty();
    let resp = resp.on_hover_text(run_tooltip(r, t, now));
    let resp = if linked { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
    if linked && resp.clicked() {
        msgs.push(Msg::OpenUrl(r.url.clone()));
    }
}

fn paint_row(ui: &Ui, rect: Rect, r: &CheckRun, now: i64, hovered: bool) {
    let p = ui.painter_at(rect);
    if hovered {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let y = rect.center().y;
    paint_run(&p, pos2(rect.left() + ICON_X, y), r.state);
    let mut right = rect.right() - 4.0;
    if let Some(time) = time_text(r, now) {
        let shown = p.text(pos2(right, y), Align2::RIGHT_CENTER, time, type_scale::font(type_scale::BODY), color::weak());
        right = shown.left() - GAP;
    }
    if r.required == Some(true) {
        let w = p.layout_no_wrap(REQUIRED.to_owned(), type_scale::font(type_scale::CAPTION), color::weak()).size().x + 12.0;
        widgets::chip(&p, right - w, y, REQUIRED, color::raised(), color::weak());
        right -= w + GAP;
    }
    let x = rect.left() + TEXT_X;
    paint_cut(&p, pos2(x, y), label_job(r), right - x);
}
