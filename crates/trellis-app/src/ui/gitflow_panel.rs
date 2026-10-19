//! The Git Flow panel, shown in place of the graph: the production and development branches,
//! then the feature, release and hotfix branches by kind with Start and Finish buttons; a set-up
//! prompt when Git Flow isn't configured.

use super::common::short;
use super::theme::color;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::gitflow::{empty_text, group_title, row_tooltip, start_blocked};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Align2, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use git2::Oid;
use trellis_core::gitflow::{Branches, FlowBranch, FlowGroup, GitFlowSettings, grouped};
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;

const NOT_SET_UP: &str = "Git Flow isn't set up in this repository.";

pub fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    ui.add_space(8.0);
    header(ui, st.settings.lang(), snap.gitflow.settings().as_ref(), msgs);
    ui.add_space(4.0);
    ui.separator();
    let margin = egui::Margin { left: 14, right: 14, top: 4, bottom: 8 };
    let Some(s) = snap.gitflow.settings() else {
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.label(RichText::new(NOT_SET_UP).color(color::weak()));
            ui.add_space(6.0);
            if ui.add(buttons::primary("Set up Git Flow…")).clicked() {
                msgs.push(configure());
            }
        });
        return;
    };
    let groups = grouped(&s, &Branches::of(snap));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            base_row(ui, snap, "Production", &s.production, msgs);
            base_row(ui, snap, "Development", &s.development, msgs);
            for g in &groups {
                ui.add_space(14.0);
                group(ui, st, snap, &s, g, msgs);
            }
        });
    });
}

fn configure() -> Msg {
    Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow))
}

/// "Features and releases start from develop · releases and hotfixes finish into main and
/// develop", with the configured branch names.
fn subtitle(s: &GitFlowSettings) -> String {
    format!("Features and releases start from {} · releases and hotfixes finish into {} and {}", s.development, s.production, s.development)
}

/// Title and subtitle on the left; Close and Configure… on the right.
fn header(ui: &mut Ui, t: Lang, s: Option<&GitFlowSettings>, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new("Git Flow").font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()));
            if let Some(s) = s {
                ui.label(RichText::new(subtitle(s)).size(type_scale::SMALL).color(color::weak()));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.0);
            let close = buttons::secondary(t.tr("common.close.button")).tip("Back to the graph (Esc)");
            if ui.add(close).clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::ClosePanel));
            }
            let config = buttons::secondary("Configure…").tip("Branches and prefixes in Preferences > Git Flow");
            if ui.add(config).clicked() {
                msgs.push(configure());
            }
        });
    });
}

/// The tip of local branch `name`, or None when it is missing or unborn.
fn tip(snap: &Snapshot, name: &str) -> Option<Oid> {
    snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid)
}

/// A clickable row with a hover fill; the caller paints into it and adds buttons over it.
fn row(ui: &mut Ui, label: &str) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(rect, 4.0, color::hover());
    }
    (rect, resp)
}

/// "Production  main  1a2b3c4", or the branch in red as missing.
fn base_row(ui: &mut Ui, snap: &Snapshot, role: &str, name: &str, msgs: &mut Vec<Msg>) {
    let exists = snap.locals.iter().any(|b| b.name == name);
    let oid = tip(snap, name);
    let (rect, resp) = row(ui, &format!("{role} {name}"));
    let p = ui.painter_at(rect);
    let y = rect.center().y;
    p.text(pos2(rect.left() + 8.0, y), Align2::LEFT_CENTER, role, type_scale::font(type_scale::SMALL), color::weak());
    let x = rect.left() + 110.0;
    let name_col = if exists { color::strong() } else { color::removed() };
    let t = p.text(pos2(x, y), Align2::LEFT_CENTER, name, type_scale::font(type_scale::BODY), name_col);
    let after = match oid {
        Some(o) => (short(o), color::faint()),
        None if !exists => ("missing".to_owned(), color::removed()),
        None => (String::new(), color::faint()),
    };
    p.text(pos2(t.right() + 12.0, y), Align2::LEFT_CENTER, after.0, type_scale::mono(type_scale::SMALL), after.1);
    if resp.clicked()
        && let Some(o) = oid
    {
        msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
    }
}

/// A kind's heading with its Start button, then its branch rows or the empty text.
fn group(ui: &mut Ui, st: &AppState, snap: &Snapshot, s: &GitFlowSettings, g: &FlowGroup, msgs: &mut Vec<Msg>) {
    let kind = g.kind;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(type_scale::section_text(ui, group_title(kind)));
        ui.label(RichText::new(g.len().to_string()).font(type_scale::font(type_scale::BODY)).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let blocked = start_blocked(snap, kind);
            let mut r = ui.add(buttons::secondary(format!("Start {}…", kind.noun())).enabled(blocked.is_none()));
            if let Some(why) = &blocked {
                r = r.on_hover_text(why);
            }
            if r.clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::StartDialog(kind)));
            }
        });
    });
    ui.add_space(2.0);
    if g.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(empty_text(s, kind)).color(color::weak()));
        });
        return;
    }
    for b in &g.branches {
        branch_row(ui, st, snap, s, g, b, msgs);
    }
}

/// The short name (✔ when checked out), the full name and the tip, with Checkout and Finish….
fn branch_row(ui: &mut Ui, st: &AppState, snap: &Snapshot, s: &GitFlowSettings, g: &FlowGroup, b: &FlowBranch, msgs: &mut Vec<Msg>) {
    let oid = tip(snap, b.name);
    let (rect, resp) = row(ui, b.name);
    let p = ui.painter_at(rect);
    let y = rect.center().y;
    let label = if b.head { format!("✔  {}", b.short) } else { b.short.to_owned() };
    let col = if b.head { color::strong() } else { color::text() };
    let t = p.text(pos2(rect.left() + 8.0, y), Align2::LEFT_CENTER, label, type_scale::font(type_scale::BODY), col);
    let t = p.text(pos2(t.right() + 12.0, y), Align2::LEFT_CENTER, b.name, type_scale::font(type_scale::SMALL), color::weak());
    if let Some(o) = oid {
        p.text(pos2(t.right() + 12.0, y), Align2::LEFT_CENTER, short(o), type_scale::mono(type_scale::SMALL), color::faint());
    }
    let builder = UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 2.0))).layout(Layout::right_to_left(Align::Center));
    ui.allocate_new_ui(builder, |ui| {
        if ui.add(buttons::secondary("Finish…")).on_hover_text(format!("Finish {} {}", g.kind.noun(), b.short)).clicked() {
            msgs.push(Msg::GitFlow(GitFlowMsg::FinishDialog(b.name.to_owned())));
        }
        let elsewhere = st.snap.as_ref().and_then(|s| crate::state::worktree_branches::checkout_blocked(s, b.name));
        let why = if b.head {
            Some("It is checked out.")
        } else if let Some(w) = elsewhere.as_deref() {
            Some(w)
        } else if st.job.is_some() {
            Some("Wait for the running job to finish.")
        } else {
            None
        };
        let r = ui.add(buttons::secondary("Checkout").enabled(why.is_none()));
        let r = match why {
            Some(w) => r.on_hover_text(w),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::Branch(crate::msg::BranchMsg::Checkout(b.name.to_owned())));
        }
    });
    let resp = resp.on_hover_text(row_tooltip(s, g.kind, b.name));
    if resp.clicked()
        && let Some(o) = oid
    {
        msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
    }
}
