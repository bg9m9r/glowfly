//! The REMOTE section of the left panel: a group row per configured remote (also remotes with
//! no branches yet) with its URLs on hover and a menu, headed by the remote's name, to fetch it,
//! view it on its hosting service, add a teammate's fork of it, fork it into the user's account,
//! edit it, copy its URLs or a link (the Copy submenu) or remove it, and under it the
//! remote-tracking branch rows. The header's + and menu add a remote; its count is the number of
//! remotes.

use super::branch_dot::{self, DOT_ROOM};
use super::common::{hint_row, menu_header, menu_item, menu_item_if};
use super::ref_visibility;
use super::remote_branch_menu::{RemoteMenu, remote_branch_menu};
use super::theme::branch_colours::line;
use super::theme::color;
use super::widgets;
use super::widgets::section_header::{self, Header, HeaderAdd, action_item};
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::remote::RemoteMsg;
use crate::state::forks::{ADD_FORK_ITEM, ForkAction, fork_menu_label, fork_target, view_item};
use crate::state::remotes::{fetch_blocked, find, tooltip};
use crate::state::section_actions::SectionAction;
use crate::state::{ForgeState, Selection};
use crate::ui::theme::type_scale;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align2, Response, Sense, pos2, vec2};
use git2::Oid;
use glowfly_core::forge::Connection;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{RemoteInfo, Snapshot};
use glowfly_core::settings::GraphFilter;

pub(super) mod dialog;
pub(super) mod fork_create;
pub(super) mod fork_pick;

/// What the section is drawn against.
pub(super) struct Ctx<'a> {
    /// The left-panel filter.
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    /// The checked-out branch's name ("" when detached).
    pub current: &'a str,
    pub fs: Option<&'a ForgeState>,
    pub graph: &'a GraphFilter,
    /// No job is running.
    pub idle: bool,
    /// The left-panel filter has text.
    pub filtering: bool,
    /// Saved connections, to recognize self-hosted services.
    pub connections: &'a [Connection],
    /// The header's Add remote.
    pub add_remote: &'a SectionAction,
    pub t: Lang,
}

pub(in crate::ui) fn section(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    open: &mut bool,
    c: Ctx,
    hover_next: &mut Option<Oid>,
    tips: &mut Vec<Oid>,
    msgs: &mut Vec<Msg>,
) {
    let slot = super::build_status::tip_slot(c.fs);
    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| action_item(ui, msgs, c.add_remote);
    let h = Header {
        key: "remote",
        title: c.t.tr("sidebar.remote.header"),
        count: Some(snap.remotes.len()),
        add: Some(HeaderAdd::send(c.add_remote)),
        menu: Some(&menu),
        t: c.t,
    };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    for (remote, branches) in &snap.remotes {
        let fallback;
        let info = match find(snap, remote) {
            Some(i) => i,
            None => {
                fallback = RemoteInfo { name: remote.clone(), ..RemoteInfo::default() };
                &fallback
            }
        };
        let r = group_row(ui, remote).on_hover_text(tooltip(info));
        r.row_menu(|ui| group_menu(ui, info, c.idle, c.connections, msgs));
        if branches.is_empty() && !c.filtering {
            hint_row(ui, c.t.tr("sidebar.remote.no_branches.hint").to_owned());
        }
        for (name, oid) in branches.iter().filter(|(n, _)| (c.shown)(n)) {
            if widgets::list_row_hidden(ui) {
                continue;
            }
            let (r, marker, cut) =
                widgets::list_row_cut(ui, widgets::NESTED_INDENT + DOT_ROOM, name, None, slot, c.sel.includes(*oid), false);
            branch_dot::paint(ui.painter(), r.rect, widgets::NESTED_INDENT, line(snap.branch_swatch(name)));
            if r.hovered() {
                *hover_next = Some(*oid);
            }
            let line = super::build_status::tip_row(ui, &r, marker, c.fs, Some(*oid), tips);
            let r = match widgets::truncate::full_tip(cut, name, line) {
                Some(tip) => r.on_hover_text(tip),
                None => r,
            };
            let full = format!("{remote}/{name}");
            if r.double_clicked() {
                msgs.push(Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.clone(), name: name.clone() }));
            } else if r.clicked() {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(*oid)));
            }
            ref_visibility::row_marker(ui, &r, widgets::NESTED_GUTTER_X, &format!("refs/remotes/{full}"), &full, c.graph, c.t, msgs);
            let m = RemoteMenu { current: c.current, fs: c.fs, graph: c.graph, sel: c.sel, t: c.t };
            r.row_menu(|ui| remote_branch_menu(ui, remote, name, *oid, snap, &m, msgs));
        }
    }
}

/// A remote's name, dim and small, on a clickable row with a hover fill.
fn group_row(ui: &mut egui::Ui, name: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), widgets::GROUP_ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let p = ui.painter_at(rect);
    if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    p.text(
        pos2(rect.left() + widgets::ROW_INDENT, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        type_scale::font(type_scale::SMALL),
        color::weak(),
    );
    resp
}

fn group_menu(ui: &mut egui::Ui, info: &RemoteInfo, idle: bool, connections: &[Connection], msgs: &mut Vec<Msg>) {
    let name = &info.name;
    let msg = |m: RemoteMsg| Msg::Remote(m);
    menu_header(ui, name);
    menu_item_if(msgs, ui, "Fetch", fetch_blocked(info, idle), msg(RemoteMsg::Fetch(name.clone())));
    let (view, why) = view_item(info, connections);
    menu_item_if(msgs, ui, view, why.as_deref(), msg(RemoteMsg::View(name.clone())));
    ui.separator();
    let why = fork_target(info, connections, ForkAction::List).err();
    menu_item_if(msgs, ui, ADD_FORK_ITEM, why.as_deref(), msg(RemoteMsg::ForkDialog(name.clone())));
    let why = fork_target(info, connections, ForkAction::Create).err();
    menu_item_if(msgs, ui, fork_menu_label(info, connections), why.as_deref(), msg(RemoteMsg::ForkOnService(name.clone())));
    ui.separator();
    menu_item(msgs, ui, "Edit…", msg(RemoteMsg::EditDialog(name.clone())));
    let (fetch, push) = (info.fetch_url(), info.separate_push_url());
    if !fetch.is_empty() || push.is_some() {
        ui.separator();
        ui.menu_button("Copy", |ui| {
            if !fetch.is_empty() {
                menu_item(msgs, ui, "Fetch URL", Msg::Copy(fetch.to_owned()));
            }
            if let Some(p) = push {
                menu_item(msgs, ui, "Push URL", Msg::Copy(p.to_owned()));
            }
            if !fetch.is_empty() {
                let link = LinkTarget::Repository { remote: Some(name.clone()) };
                menu_item(msgs, ui, "Link", Msg::DeepLink(DeepLinkMsg::Copy(link)));
            }
        });
    }
    ui.separator();
    menu_item(msgs, ui, "Remove…", Msg::Remote(RemoteMsg::RemoveDialog(name.clone())));
}
