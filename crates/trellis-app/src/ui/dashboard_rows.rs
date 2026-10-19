//! The dashboard's rows: a repository row (opens its preferred clone), pull request rows (two
//! lines, with the build marker, votes and the shared tooltip) and issue rows, each with its
//! context menu.

use super::common::{menu_item, menu_item_if, state_color};
use super::theme::color;
use super::widgets::{self, Part, truncate::full_tip};
use crate::msg::{DashboardMsg, Msg};
use crate::state::AppState;
use crate::state::dashboard::{Dashboard, TargetData, pr_label, target_name};
use crate::state::dashboard_rows::issue_text;
use crate::state::forge_filters;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Ui, pos2, vec2};
use trellis_core::forge::{self, Issue, Provider, PullRequest};

/// Indent of pull request and issue rows (as in the sidebar).
const INDENT: f32 = widgets::ROW_INDENT;
const NO_LINK: &str = "The service gave no link";

fn dash(m: DashboardMsg) -> Msg {
    Msg::Dashboard(m)
}

/// "widgets, widgets-fork +1": at most two folder names, then how many more.
fn folders_text(names: &[&str]) -> String {
    let mut s = names.iter().take(2).copied().collect::<Vec<_>>().join(", ");
    if names.len() > 2 {
        s.push_str(&format!(" +{}", names.len() - 2));
    }
    s
}

/// "Open repository": the preferred clone, when there is one.
fn open_repo_item(ui: &mut Ui, st: &AppState, data: &TargetData, msgs: &mut Vec<Msg>) {
    if let Some(l) = Dashboard::preferred_local(data, st) {
        menu_item(msgs, ui, "Open repository", dash(DashboardMsg::OpenRepo(l.path.clone())));
    }
}

pub(super) fn repo_row(ui: &mut Ui, st: &AppState, data: &TargetData, msgs: &mut Vec<Msg>) {
    let names = data.folder_names();
    let right = folders_text(&names);
    let name = target_name(&data.target);
    let (r, _, cut) = widgets::list_row_cut(ui, widgets::PAD_X, &name, Some((&[Part::Text(&right)][..], color::weak())), 0.0, false, true);
    let preferred = Dashboard::preferred_local(data, st);
    if r.clicked()
        && let Some(l) = preferred
    {
        msgs.push(dash(DashboardMsg::OpenRepo(l.path.clone())));
    }
    let paths: Vec<String> = data.locals.iter().map(|l| trellis_core::native_path::shown(&l.path)).collect();
    let r = match full_tip(cut, &name, Some(paths.join("\n"))) {
        Some(tip) => r.on_hover_text(tip),
        None => r,
    };
    let page =
        data.locals.iter().filter(|l| !l.remote_url.is_empty()).find_map(|l| forge::web_page(&l.remote_url, &st.settings.connections).ok());
    r.context_menu(|ui| {
        for l in data.locals.iter().take(5) {
            menu_item(msgs, ui, format!("Open {}", l.name), dash(DashboardMsg::OpenRepo(l.path.clone())));
        }
        if let Some(p) = &page {
            let service = if p.service.is_empty() { "the web".to_owned() } else { p.service.clone() };
            menu_item(msgs, ui, format!("Open on {service}"), Msg::OpenUrl(p.url.clone()));
            menu_item(msgs, ui, "Copy link", Msg::Copy(p.url.clone()));
        }
    });
}

/// A two-line row: `title` (with dim `right` text and a marker slot at the right), and `sub`
/// below it in weak text, each cut with "…". Returns the response, the marker's rect and
/// whether the title was cut.
fn two_line_row(ui: &mut Ui, title: &str, sub: &str, right: (&str, Color32), slot: f32) -> (Response, Rect, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    let p = ui.painter_at(rect);
    if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let top = rect.top() + 12.0;
    let g = p.layout_no_wrap(right.0.to_owned(), type_scale::font(type_scale::SMALL), right.1);
    let right_w = g.size().x + widgets::PAD_X + 8.0 + slot;
    p.galley(pos2(rect.right() - g.size().x - widgets::PAD_X - slot, top - g.size().y / 2.0), g, right.1);
    let clip = Rect::from_min_max(pos2(rect.left() + INDENT, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    let line = |text: &str, y: f32, size: f32, col: Color32| {
        widgets::paint_cut_text(&p.with_clip_rect(clip), pos2(clip.left(), y), text, FontId::proportional(size), col, clip.width())
    };
    let cut = line(title, top, 13.0, color::text()).cut;
    line(sub, rect.top() + 29.0, 11.5, color::weak());
    let marker =
        Rect::from_min_max(pos2(rect.right() - widgets::PAD_X - slot, rect.top()), pos2(rect.right() - widgets::PAD_X, rect.top() + 24.0));
    (resp, marker, cut)
}

pub(super) fn pr_row(ui: &mut Ui, st: &AppState, data: &TargetData, pr: &PullRequest, slot: f32, msgs: &mut Vec<Msg>) {
    let t = &data.target;
    let title = if pr.draft { format!("[Draft] {}", pr.title) } else { pr.title.clone() };
    let votes = pr.approvals();
    let right = format!("{}{}", pr_label(t, pr.id), if votes > 0 { format!(" ✔{votes}") } else { String::new() });
    // The font has no arrow glyph.
    let mut sub = format!("{} into {} · by {}", pr.source, pr.target, pr.author);
    if pr.reviewers.iter().any(|r| r.state.starts_with("changes")) {
        sub.push_str(" · changes requested");
    }
    // The pull request's tooltip has the whole title, so it stays the only one.
    let (r, marker, _) = two_line_row(ui, &title, &sub, (&right, color::purple()), slot);
    if let Some(c) = &pr.checks
        && slot > 0.0
    {
        super::build_status::paint_marker(&ui.painter_at(r.rect), marker.center(), c.state);
    }
    let r = r.on_hover_text(super::pr_menu_items::pr_tooltip(pr));
    let show = Dashboard::pr_host_local(data).map(|l| l.path.clone());
    let open_pr = |path: &std::path::Path| dash(DashboardMsg::OpenPr { path: path.to_path_buf(), target: t.clone(), id: pr.id });
    let link = (!pr.url.is_empty()).then_some(pr.url.as_str());
    if r.double_clicked() {
        match (&show, link) {
            (Ok(path), _) => msgs.push(open_pr(path)),
            (Err(_), Some(u)) => msgs.push(Msg::OpenUrl(u.to_owned())),
            (Err(_), None) => {}
        }
    }
    r.context_menu(|ui| {
        let label = format!("Show {}", t.provider.pr_noun());
        match &show {
            Ok(path) => menu_item(msgs, ui, label, open_pr(path)),
            Err(why) => menu_item_if(msgs, ui, label, Some(why), open_pr(std::path::Path::new(""))),
        }
        let blocked = link.is_none().then_some(NO_LINK);
        menu_item_if(msgs, ui, "Open in browser", blocked, Msg::OpenUrl(pr.url.clone()));
        super::build_status::pr_menu_item(ui, pr, msgs);
        menu_item_if(msgs, ui, "Copy link", blocked, Msg::Copy(pr.url.clone()));
        open_repo_item(ui, st, data, msgs);
    });
}

pub(super) fn issue_row(ui: &mut Ui, st: &AppState, data: &TargetData, issue: &Issue, msgs: &mut Vec<Msg>) {
    let t = &data.target;
    // The tooltip has the whole title, so a cut one needs no hover of its own.
    let right = [Part::Text(&issue.state)];
    let (r, _, _) =
        widgets::list_row_cut(ui, INDENT, &issue_text(t, issue), Some((&right[..], state_color(&issue.state))), 0.0, false, false);
    let assignee = issue.assignee.as_deref().unwrap_or("unassigned");
    let mut tip = format!("{} {}: {}\n{} · {assignee}", issue.kind, issue.key, issue.title, issue.state);
    if let Some(line) = forge_filters::facet_line(&issue.facets) {
        tip.push_str(&format!("\n{line}"));
    }
    let r = r.on_hover_text(tip);
    let link = (!issue.url.is_empty()).then_some(issue.url.as_str());
    if r.double_clicked()
        && let Some(u) = link
    {
        msgs.push(Msg::OpenUrl(u.to_owned()));
    }
    r.context_menu(|ui| {
        let open = if t.provider == Provider::Trello { "View card in Trello" } else { "Open in browser" };
        let blocked = link.is_none().then_some(NO_LINK);
        menu_item_if(msgs, ui, open, blocked, Msg::OpenUrl(issue.url.clone()));
        menu_item_if(msgs, ui, "Copy link", blocked, Msg::Copy(issue.url.clone()));
        open_repo_item(ui, st, data, msgs);
    });
}
