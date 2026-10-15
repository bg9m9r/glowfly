//! The TAGS section of the left panel: one row per tag (a click jumps to its commit) and the row
//! menu: checkout, push to each remote or all of them, delete locally, and delete from each
//! remote or all of them (asked first).

use super::common::{menu_item, menu_item_if};
use super::widgets;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::Selection;
use crate::state::rebase::JOB_RUNNING;
use crate::state::tags::delete_remote_confirm;
use crate::tag::TagMsg;
use eframe::egui;
use trellis_core::repo::Snapshot;

/// What the rows are drawn against: the left-panel filter, the selection, and whether no job is
/// running (the remote items are disabled while one is).
pub(super) struct Rows<'a> {
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub idle: bool,
}

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, rows: Rows, msgs: &mut Vec<Msg>) {
    let Rows { shown, sel, idle } = rows;
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
        r.context_menu(|ui| menu(ui, name, snap, idle, msgs));
    }
}

fn menu(ui: &mut egui::Ui, name: &str, snap: &Snapshot, idle: bool, msgs: &mut Vec<Msg>) {
    let remotes: Vec<String> = snap.remotes.iter().map(|(r, _)| r.clone()).collect();
    let blocked = (!idle).then_some(JOB_RUNNING);
    let tag = || name.to_owned();
    menu_item(msgs, ui, "Checkout (detached)", Msg::git("Checkout", &["checkout", &format!("refs/tags/{name}")]));
    if !remotes.is_empty() {
        ui.separator();
        for r in &remotes {
            menu_item_if(msgs, ui, format!("Push to {r}"), blocked, Msg::Tag(TagMsg::Push { tag: tag(), remotes: vec![r.clone()] }));
        }
        if remotes.len() > 1 {
            menu_item_if(msgs, ui, "Push to all remotes", blocked, Msg::Tag(TagMsg::Push { tag: tag(), remotes: remotes.clone() }));
        }
    }
    ui.separator();
    let delete = Msg::git("Delete tag", &["tag", "-d", name]);
    menu_item(msgs, ui, "Delete locally", Msg::confirm("Delete tag", format!("Delete local tag {name}?"), "Delete", delete));
    if !remotes.is_empty() {
        ui.separator();
        for r in &remotes {
            menu_item_if(msgs, ui, format!("Delete from {r}"), blocked, delete_remote_confirm(name, vec![r.clone()]));
        }
        if remotes.len() > 1 {
            menu_item_if(msgs, ui, "Delete from all remotes", blocked, delete_remote_confirm(name, remotes));
        }
    }
}
