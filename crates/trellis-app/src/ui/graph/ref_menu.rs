//! A graph row's right-click menu: on its BRANCH / TAG chips, the menu of the ref under them
//! (a submenu per ref when the row has several), elsewhere the commit menu, or the
//! multi-selection menu for a row of the selection.

use crate::msg::Msg;
use crate::state::{ForgeState, Selection};
use crate::ui::branch_menu::local_branch_menu;
use crate::ui::menus::{RemoteMenu, commit_menu, multi_commit_menu, remote_branch_menu};
use crate::ui::tags::tag_menu;
use eframe::egui::{self, Rect, Response};
use trellis_core::repo::{CommitInfo, RefKind, RefLabel, Snapshot};
use trellis_core::settings::Settings;

/// What a row's menu is drawn against.
pub(super) struct RowMenu<'a> {
    pub commit: &'a CommitInfo,
    /// Where the row's ref chips are (the first chip and the "+N" one), if it has any.
    pub labels_hit: Option<Rect>,
    pub snap: &'a Snapshot,
    pub selection: &'a Selection,
    pub settings: &'a Settings,
    pub forge: Option<&'a ForgeState>,
    /// No job is running.
    pub idle: bool,
}

/// Attaches the row's menu to `resp`. Where the right-click happened is kept until the menu
/// closes, so the menu doesn't change when the pointer moves on.
pub(super) fn context_menu(ui: &mut egui::Ui, resp: &Response, m: RowMenu, msgs: &mut Vec<Msg>) {
    let id = ui.id().with(("graph-ref-menu", m.commit.oid));
    if resp.secondary_clicked() {
        let on_labels = resp.interact_pointer_pos().zip(m.labels_hit).is_some_and(|(p, r)| r.contains(p));
        ui.ctx().data_mut(|d| d.insert_temp(id, on_labels));
    }
    resp.context_menu(|ui| {
        let on_labels = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        match m.snap.refs.get(&m.commit.oid).filter(|l| on_labels && !l.is_empty()) {
            Some(labels) => labels_menu(ui, labels, &m, msgs),
            None => match m.selection {
                Selection::Commits(s) if s.contains(m.commit.oid) => multi_commit_menu(ui, m.settings.lang(), s, m.snap, msgs),
                _ => commit_menu(ui, m.commit, m.snap, msgs),
            },
        }
    });
}

/// One label's menu straight away; several get a submenu each, in label order.
fn labels_menu(ui: &mut egui::Ui, labels: &[RefLabel], m: &RowMenu, msgs: &mut Vec<Msg>) {
    if let [only] = labels {
        return label_menu(ui, only, m, msgs);
    }
    for l in labels {
        let title = match l.kind {
            RefKind::Local => format!("Branch {}", l.name),
            RefKind::Remote => format!("Remote {}", l.name),
            RefKind::Tag => format!("Tag {}", l.name),
            RefKind::Head => "HEAD".to_owned(),
        };
        ui.menu_button(title, |ui| label_menu(ui, l, m, msgs));
    }
}

/// The menu of the ref `l`: a local branch's or remote branch's from the sidebar, a tag's, or
/// the commit menu (a detached HEAD, or a branch the snapshot doesn't list).
fn label_menu(ui: &mut egui::Ui, l: &RefLabel, m: &RowMenu, msgs: &mut Vec<Msg>) {
    let snap = m.snap;
    let filter = || m.settings.graph_filter_for(&snap.real_workdir);
    match l.kind {
        RefKind::Local => {
            if let Some(b) = snap.locals.iter().find(|b| b.name == l.name) {
                return local_branch_menu(ui, b, snap, m.forge, &filter(), m.selection, msgs);
            }
        }
        RefKind::Remote => {
            let remote = snap
                .remotes
                .iter()
                .map(|(r, _)| r.as_str())
                .filter(|r| l.name.strip_prefix(r).is_some_and(|rest| rest.starts_with('/')))
                .max_by_key(|r| r.len());
            if let Some(remote) = remote {
                let name = &l.name[remote.len() + 1..];
                let current = snap.head_branch.as_deref().unwrap_or_default();
                let graph = filter();
                let c = RemoteMenu { current, fs: m.forge, graph: &graph, sel: m.selection };
                return remote_branch_menu(ui, remote, name, m.commit.oid, snap, &c, msgs);
            }
        }
        RefKind::Tag => return tag_menu(ui, &l.name, snap, &filter(), m.idle, msgs),
        RefKind::Head => {}
    }
    commit_menu(ui, m.commit, snap, msgs);
}
