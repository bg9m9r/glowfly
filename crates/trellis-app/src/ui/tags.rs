//! The TAGS section of the left panel: one row per tag (a click jumps to its commit) and the row
//! menu, headed by the tag's name: checkout, annotate a lightweight tag, push to a remote or all
//! of them, hide or solo it in the graph, copy its name or a link, delete locally, and delete
//! from a remote or all of them (asked first).

use super::common::{menu_header, menu_item, menu_item_if};
use super::ref_visibility;
use super::widgets;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::Selection;
use crate::state::rebase::JOB_RUNNING;
use crate::state::tags::delete_remote_confirm;
use crate::tag::TagMsg;
use eframe::egui;
use trellis_core::repo::Snapshot;
use trellis_core::settings::GraphFilter;

const ANNOTATE_HINT: &str = "Replace this lightweight tag with an annotated tag on the same commit";

/// What the rows are drawn against: the left-panel filter, the selection, whether no job is
/// running (the remote items are disabled while one is), and the graph filter (hidden and soloed
/// tags).
pub(super) struct Rows<'a> {
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub idle: bool,
    pub graph: &'a GraphFilter,
}

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, rows: Rows, msgs: &mut Vec<Msg>) {
    let Rows { shown, sel, idle, graph } = rows;
    widgets::section_header(ui, "TAGS", Some(snap.tags.len()), open);
    if !*open {
        return;
    }
    for (name, oid) in snap.tags.iter().filter(|(n, _)| shown(n)) {
        if widgets::list_row_hidden(ui) {
            continue;
        }
        let r = widgets::list_row(ui, 26.0, name, None, *sel == Selection::Commit(*oid), false);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::JumpTo(*oid)));
        }
        let full = format!("refs/tags/{name}");
        ref_visibility::row_marker(ui, &r, 13.0, &full, name, graph, msgs);
        r.context_menu(|ui| tag_menu(ui, name, snap, graph, idle, msgs));
    }
}

/// A tag's menu (also on its label in the graph).
pub(in crate::ui) fn tag_menu(ui: &mut egui::Ui, name: &str, snap: &Snapshot, graph: &GraphFilter, idle: bool, msgs: &mut Vec<Msg>) {
    let remotes: Vec<String> = snap.remotes.iter().map(|(r, _)| r.clone()).collect();
    let blocked = (!idle).then_some(JOB_RUNNING);
    let tag = || name.to_owned();
    menu_header(ui, name);
    menu_item(msgs, ui, "Checkout (detached)", Msg::Tag(TagMsg::Checkout { tag: tag() }));
    // Stays enabled while a job runs, like Delete locally: update checks when the job would start.
    if snap.lightweight_tags.contains(name) && ui.button("Annotate…").on_hover_text(ANNOTATE_HINT).clicked() {
        msgs.push(Msg::Tag(TagMsg::OpenAnnotate { tag: tag() }));
        ui.close_menu();
    }
    if !remotes.is_empty() {
        ui.separator();
        push_menu(ui, name, &remotes, blocked, msgs);
    }
    // Never disabled: a tag is never the checked-out branch, and these only change settings.
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/tags/{name}"), graph, false, msgs);
    ui.separator();
    ui.menu_button("Copy", |ui| {
        menu_item(msgs, ui, "Name", Msg::Copy(tag()));
        menu_item(msgs, ui, "Link", Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::Tag(tag()))));
    });
    ui.separator();
    let delete = Msg::git("Delete tag", &["tag", "-d", name]);
    menu_item(msgs, ui, "Delete locally…", Msg::confirm("Delete tag", format!("Delete local tag {name}?"), "Delete", delete));
    if !remotes.is_empty() {
        delete_remote_menu(ui, name, &remotes, blocked, msgs);
    }
}

/// "Push to": one entry per remote and, with more than one, all of them.
fn push_menu(ui: &mut egui::Ui, name: &str, remotes: &[String], blocked: Option<&str>, msgs: &mut Vec<Msg>) {
    ui.menu_button("Push to", |ui| {
        for r in remotes {
            menu_item_if(msgs, ui, r, blocked, Msg::Tag(TagMsg::Push { tag: name.to_owned(), remotes: vec![r.clone()] }));
        }
        if remotes.len() > 1 {
            ui.separator();
            menu_item_if(msgs, ui, "All remotes", blocked, Msg::Tag(TagMsg::Push { tag: name.to_owned(), remotes: remotes.to_vec() }));
        }
    });
}

/// "Delete from remote": one entry per remote and, with more than one, all of them. Each asks
/// first.
fn delete_remote_menu(ui: &mut egui::Ui, name: &str, remotes: &[String], blocked: Option<&str>, msgs: &mut Vec<Msg>) {
    ui.menu_button("Delete from remote", |ui| {
        for r in remotes {
            menu_item_if(msgs, ui, format!("{r}…"), blocked, delete_remote_confirm(name, vec![r.clone()]));
        }
        if remotes.len() > 1 {
            ui.separator();
            menu_item_if(msgs, ui, "All remotes…", blocked, delete_remote_confirm(name, remotes.to_vec()));
        }
    });
}
