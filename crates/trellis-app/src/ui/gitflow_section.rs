//! The GIT FLOW section of the left panel: Features, Releases and Hotfixes groups with a Start
//! '+' and menu each, and the local flow branches under them (prefix stripped). A click jumps to
//! the tip, a double-click checks out, the row menu finishes the branch or does any local branch
//! action; the header's + offers the three Start items, and its menu opens the panel, starts a
//! branch or configures Git Flow. Also the issue menu's Start Git Flow items.

use super::branch_menu::local_branch_menu;
use super::common::{menu_item, menu_item_if};
use super::ref_visibility;
use super::theme::color;
use super::widgets;
use super::widgets::section_header::{self, AddAction, Header, HeaderAdd};
use crate::gitflow::GitFlowMsg;
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::gitflow::{group_title, row_tooltip, start_blocked};
use crate::state::{ForgeState, PrefPage, Selection};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align2, Rect, Response, Sense, Stroke, pos2, vec2};
use git2::Oid;
use trellis_core::forge::{self, Issue};
use trellis_core::gitflow::{Branches, FlowGroup, FlowKind, GitFlowSettings, grouped, total};
use trellis_core::repo::Snapshot;
use trellis_core::settings::GraphFilter;

/// What the rows are drawn against.
pub(super) struct Rows<'a> {
    /// The left-panel filter.
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub fs: Option<&'a ForgeState>,
    pub graph: &'a GraphFilter,
    /// The left-panel filter has text.
    pub filtering: bool,
}

pub(super) fn section(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    s: &GitFlowSettings,
    open: &mut bool,
    rows: Rows,
    hover_next: &mut Option<Oid>,
    msgs: &mut Vec<Msg>,
) {
    let groups = grouped(s, &Branches::of(snap));
    let starts = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
        for k in FlowKind::ALL {
            start_item(ui, snap, k, msgs);
        }
    };
    let add = HeaderAdd {
        name: "Start Git Flow branch".into(),
        tip: Some("Start a feature, release or hotfix".into()),
        blocked: None,
        action: AddAction::Menu(&starts),
    };
    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| header_menu(ui, snap, msgs);
    let h = Header { title: "Git Flow", count: Some(total(&groups)), add: Some(add), menu: Some(&menu) };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    for g in &groups {
        let matching: Vec<_> = g.branches.iter().filter(|b| (rows.shown)(b.name)).collect();
        if rows.filtering && matching.is_empty() {
            continue;
        }
        group(ui, snap, s, g, msgs);
        for b in matching {
            if widgets::list_row_hidden(ui) {
                continue;
            }
            let Some(info) = snap.locals.iter().find(|i| i.name == b.name) else { continue };
            let label = if b.head { format!("✔  {}", b.short) } else { b.short.to_owned() };
            let selected = info.oid.is_some_and(|o| *rows.sel == Selection::Commit(o));
            let worktree = crate::state::worktree_branches::elsewhere(snap, b.name);
            let right = worktree.map(|_| [widgets::Part::Text("worktree")]);
            let (r, _, cut) = widgets::list_row_cut(
                ui,
                widgets::NESTED_INDENT,
                &label,
                right.as_ref().map(|p| (&p[..], color::weak())),
                0.0,
                selected,
                b.head,
            );
            if r.hovered() {
                *hover_next = info.oid;
            }
            let tip = row_tooltip(s, g.kind, b.name);
            let tip = match worktree {
                Some(wt) => format!(
                    "{tip}
{}",
                    crate::state::worktree_branches::local_tip_line(wt)
                ),
                None => tip,
            };
            let r = r.on_hover_text(widgets::truncate::full_tip(cut, &label, Some(tip)).unwrap_or_default());
            if r.double_clicked() {
                msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.to_owned())));
            } else if r.clicked()
                && let Some(o) = info.oid
            {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
            }
            ref_visibility::row_marker(ui, &r, widgets::NESTED_GUTTER_X, &format!("refs/heads/{}", b.name), b.name, rows.graph, msgs);
            r.context_menu(|ui| {
                let finish = format!("Finish {} {}…", g.kind.noun(), b.short);
                menu_item(msgs, ui, finish, Msg::GitFlow(GitFlowMsg::FinishDialog(b.name.to_owned())));
                ui.separator();
                local_branch_menu(ui, info, snap, rows.fs, rows.graph, rows.sel, msgs);
            });
        }
    }
}

/// Open the panel, Start per kind, Configure.
fn header_menu(ui: &mut egui::Ui, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    menu_item(msgs, ui, "Open Git Flow panel", Msg::GitFlow(GitFlowMsg::OpenPanel));
    ui.separator();
    for k in FlowKind::ALL {
        start_item(ui, snap, k, msgs);
    }
    ui.separator();
    menu_item(msgs, ui, "Configure Git Flow…", Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
}

/// "Start feature…", disabled while the base branch is missing.
fn start_item(ui: &mut egui::Ui, snap: &Snapshot, kind: FlowKind, msgs: &mut Vec<Msg>) {
    let blocked = start_blocked(snap, kind);
    menu_item_if(msgs, ui, format!("Start {}…", kind.noun()), blocked.as_deref(), Msg::GitFlow(GitFlowMsg::StartDialog(kind)));
}

/// A kind's group row with its Start '+' and menu.
fn group(ui: &mut egui::Ui, snap: &Snapshot, s: &GitFlowSettings, g: &FlowGroup, msgs: &mut Vec<Msg>) {
    let kind = g.kind;
    let blocked = start_blocked(snap, kind);
    let (row, plus) = group_row(ui, group_title(kind), g.len(), &format!("Start {}", kind.noun()), blocked.is_none());
    row.context_menu(|ui| start_item(ui, snap, kind, msgs));
    let tip = format!("Start a {} from {}", kind.noun(), s.base(kind));
    let plus = match &blocked {
        Some(why) => plus.on_hover_text(why),
        None => plus.on_hover_text(tip),
    };
    if plus.clicked() && blocked.is_none() {
        msgs.push(Msg::GitFlow(GitFlowMsg::StartDialog(kind)));
    }
}

/// A group's title, dim and small, with its count and a '+' (named `plus_name`) at the right
/// end. Returns the row's and the '+''s responses.
fn group_row(ui: &mut egui::Ui, title: &str, count: usize, plus_name: &str, enabled: bool) -> (Response, Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), widgets::GROUP_ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    let p = ui.painter_at(rect);
    if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let font = type_scale::font(type_scale::SMALL);
    let t = p.text(pos2(rect.left() + widgets::ROW_INDENT, rect.center().y), Align2::LEFT_CENTER, title, font.clone(), color::weak());
    p.text(pos2(t.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, count.to_string(), font, color::faint());

    let c = pos2(rect.right() - widgets::PAD_X, rect.center().y);
    let plus = ui.interact(Rect::from_center_size(c, vec2(18.0, 18.0)), resp.id.with("start"), Sense::click());
    plus.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, plus_name));
    if enabled && plus.hovered() {
        p.rect_filled(plus.rect, 3.0, color::hover());
    }
    let col = if !enabled {
        color::faint()
    } else if plus.hovered() {
        color::text()
    } else {
        color::weak()
    };
    let s = Stroke::new(1.5_f32, col);
    p.line_segment([pos2(c.x - 4.5, c.y), pos2(c.x + 4.5, c.y)], s);
    p.line_segment([pos2(c.x, c.y - 4.5), pos2(c.x, c.y + 4.5)], s);
    let plus = if enabled { plus.on_hover_cursor(egui::CursorIcon::PointingHand) } else { plus };
    (resp, plus)
}

/// The issue menu's "Start Git Flow <kind> from <issue noun>…" items, when Git Flow is set up.
pub(super) fn issue_items(ui: &mut egui::Ui, snap: &Snapshot, f: &ForgeState, issue: &Issue, msgs: &mut Vec<Msg>) {
    if !snap.gitflow.is_initialized() {
        return;
    }
    for k in FlowKind::ALL {
        let label = format!("Start Git Flow {} from {}…", k.noun(), f.issue_noun());
        let m = Msg::GitFlow(GitFlowMsg::StartFromIssue { kind: k, name: forge::branch_name_for(issue) });
        menu_item_if(msgs, ui, label, start_blocked(snap, k).as_deref(), m);
    }
}
