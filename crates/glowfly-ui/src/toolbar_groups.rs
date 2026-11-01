//! The toolbar's middle groups as buttons (Undo and Redo; Pull and Push; Branch, Stash and Pop;
//! the pull request group; Terminal and Folder), the split buttons' menus, which the More menu
//! shows as submenus too, and the groups' full widths for the fold decision (`toolbar_fold.rs`).

use super::menus::{split, terminal_button};
use super::tips;
use crate::msg::{BranchMsg, ForgeMsg, JobMsg, Msg};
use crate::state::toolbar_tips as rules;
use crate::state::{AppState, ForgeState};
use crate::ui::widgets::menu::separator;
use crate::ui::widgets::reason::{self, ItemState};
use crate::ui::widgets::{self, Icon, tooltip::attach};
use eframe::egui;
use glowfly_core::repo::Snapshot;

/// The pull menu's variants: the item's text, and the job's label (English: it is also log
/// and undo text) with its arguments.
pub(super) fn pull_variants(st: &AppState) -> [(&'static str, &'static str, &'static [&'static str]); 3] {
    let t = st.settings.lang();
    [
        (t.tr("toolbar.pull_ff"), "Pull (fast-forward if possible)", &["pull", "--ff"]),
        (t.tr("toolbar.pull_ff_only"), "Pull (fast-forward only)", &["pull", "--ff-only"]),
        (t.tr("toolbar.pull_rebase"), "Pull (rebase)", &["pull", "--rebase"]),
    ]
}

/// The pull menu's items: Fetch all and the pull variants. A blocked entry shows its reason; a
/// ready one, what it would do.
pub(super) fn pull_items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (t, keys) = (st.settings.lang(), &st.keymap);
    let why = rules::fetch_blocked(st);
    let r = reason::menu_button(ui, t.tr("toolbar.fetch_all"), "", ItemState::blocked_by(why.as_deref()));
    let r = if why.is_none() { attach(r, keys, t, || tips::fetch_all(st)) } else { r };
    if r.clicked() {
        msgs.push(Msg::git("Fetch all", &["fetch", "--all", "--prune"]));
    }
    for (text, label, a) in pull_variants(st) {
        let why = rules::pull_variant_blocked(st, text);
        let r = reason::menu_button(ui, text, "", ItemState::blocked_by(why.as_deref()));
        let r = if why.is_none() { attach(r, keys, t, || tips::pull_variant(st, text)) } else { r };
        if r.clicked() {
            msgs.push(Msg::git(label, a));
        }
    }
}

/// The push menu's items: Publish without a remote, the pull request push with a hosting
/// service, and Force push, destructive, so last and set apart.
pub(super) fn push_items(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, host: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let busy = rules::busy_blocked(st);
    if snap.remotes.is_empty() && reason::menu_button(ui, t.tr("toolbar.publish"), "", ItemState::blocked_by(busy.as_deref())).clicked() {
        msgs.push(Msg::Publish(crate::msg::PublishMsg::PublishDialog));
    }
    if let Some(fs) = host {
        separator(ui);
        crate::ui::pr_menu_items::push_pr_item(ui, t, snap, fs, st.job.is_none(), msgs);
    }
    separator(ui);
    let force = ItemState::danger_unless(busy.as_deref());
    // Its confirmation warns plainly before overwriting the default branch or a Git Flow base.
    if reason::menu_button(ui, t.tr("toolbar.force_push"), "", force).clicked() {
        msgs.push(crate::state::branches::push::head_force_confirm(t, snap));
    }
}

/// Whether the pull request service calls them merge requests.
pub(super) fn merge_requests(fs: &ForgeState) -> bool {
    fs.pr_noun() == "merge request"
}

fn tool(ui: &mut egui::Ui, st: &AppState, icon: Icon, (label, name): (&str, &str), enabled: bool) -> egui::Response {
    widgets::tool_button(ui, icon, label, name, enabled, st.settings.toolbar_labels)
}

/// Undo and Redo.
pub(super) fn undo(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (t, keys, idle) = (st.settings.lang(), &st.keymap, st.job.is_none());
    widgets::tool_group(ui, |ui| {
        let r =
            tool(ui, st, Icon::Undo, (t.tr("toolbar.undo.button"), t.tr("toolbar.undo.name")), idle && !st.session.undo.undo.is_empty());
        if attach(r, keys, t, || tips::undo(st)).clicked() {
            msgs.push(Msg::Job(JobMsg::Undo));
        }
        let r =
            tool(ui, st, Icon::Redo, (t.tr("toolbar.redo.button"), t.tr("toolbar.redo.name")), idle && !st.session.undo.redo.is_empty());
        if attach(r, keys, t, || tips::redo(st)).clicked() {
            msgs.push(Msg::Job(JobMsg::Redo));
        }
    });
}

/// Pull and Push, each with its menu.
pub(super) fn sync(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, host: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let (t, keys, labels) = (st.settings.lang(), &st.keymap, st.settings.toolbar_labels);
    widgets::tool_group(ui, |ui| {
        let pull = (Icon::Pull, t.tr("toolbar.pull.button"), t.tr("toolbar.pull.name"));
        let live = rules::fetch_blocked(st).is_none() || pull_variants(st).iter().any(|v| rules::pull_variant_blocked(st, v.0).is_none());
        let menu = (t.tr("toolbar.pull_menu"), 240.0, live);
        let r = split(ui, "pull-menu", pull, rules::pull_blocked(st).is_none(), labels, menu, |ui| pull_items(ui, st, msgs));
        if attach(r, keys, t, || tips::pull(st)).clicked() {
            msgs.push(Msg::git("Pull", &["pull", "--ff"]));
        }
        let push = (Icon::Push, t.tr("toolbar.push.button"), t.tr("toolbar.push.name"));
        let menu = (t.tr("toolbar.push_menu"), if host.is_some() { 260.0 } else { 200.0 }, rules::busy_blocked(st).is_none());
        let r = split(ui, "push-menu", push, rules::push_blocked(st).is_none(), labels, menu, |ui| push_items(ui, st, snap, host, msgs));
        if attach(r, keys, t, || tips::push(st)).clicked() {
            msgs.push(Msg::Job(JobMsg::Push { force: false }));
        }
    });
}

/// Branch, Stash and Pop.
pub(super) fn branch(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let (t, keys) = (st.settings.lang(), &st.keymap);
    widgets::tool_group(ui, |ui| {
        let r =
            tool(ui, st, Icon::Branch, (t.tr("toolbar.branch.button"), t.tr("toolbar.branch.name")), rules::branch_blocked(st).is_none());
        if attach(r, keys, t, || tips::branch(st)).clicked() {
            msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
        }
        let r = tool(ui, st, Icon::Stash, (t.tr("toolbar.stash.button"), t.tr("toolbar.stash.name")), rules::stash_blocked(st).is_none());
        if attach(r, keys, t, || tips::stash(st)).clicked() {
            msgs.push(Msg::git("Stash", &["stash", "push", "-u"]));
        }
        let r = tool(ui, st, Icon::Pop, (t.tr("toolbar.pop.button"), t.tr("toolbar.pop.name")), rules::pop_blocked(st).is_none());
        if attach(r, keys, t, || tips::pop(st)).clicked() {
            msgs.extend(crate::state::stash_confirm::pop_latest(snap, t));
        }
    });
}

/// The pull request group: create one for this branch, with its menu.
pub(super) fn pr(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, fs: &ForgeState, msgs: &mut Vec<Msg>) {
    let (t, keys, idle) = (st.settings.lang(), &st.keymap, st.job.is_none());
    widgets::tool_group(ui, |ui| {
        let service = fs.host_provider().map(glowfly_core::forge::Provider::name).unwrap_or_default();
        let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
        let name =
            if merge_requests(fs) { t.trf("toolbar.pr.name_merge_request", args) } else { t.trf("toolbar.pr.name_pull_request", args) };
        // The name starts with the caption, so speech input finds the button by what it shows.
        let pr = (Icon::PullRequest, t.tr("toolbar.pr.button"), name.as_str());
        let menu_name = if merge_requests(fs) { t.tr("toolbar.pr_menu.merge_request") } else { t.tr("toolbar.pr_menu.pull_request") };
        let menu = (menu_name, 260.0, crate::ui::pr_menu_items::pr_menu_live(t, snap, fs, idle));
        let r = split(ui, "pr-menu", pr, rules::pr_blocked(st).is_none(), st.settings.toolbar_labels, menu, |ui| {
            crate::ui::pr_menu_items::pr_menu(ui, t, snap, fs, idle, msgs);
        });
        if attach(r, keys, t, || tips::pr(st, fs)).clicked() {
            msgs.extend(create_pr(snap));
        }
    });
}

/// The pull request button's action: the create dialog for the checked-out branch.
pub(super) fn create_pr(snap: &Snapshot) -> Option<Msg> {
    let b = snap.head_branch.as_ref()?;
    Some(Msg::Forge(ForgeMsg::CreatePrDialog { source: b.clone(), target: None }))
}

/// Terminal and Folder.
pub(super) fn files(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let (t, keys) = (st.settings.lang(), &st.keymap);
    widgets::tool_group(ui, |ui| {
        terminal_button(ui, st, st.settings.toolbar_labels, crate::ui::terminal::right_click_item(t, &st.settings), msgs);
        let r = tool(ui, st, Icon::Folder, (t.tr("toolbar.folder.button"), t.tr("toolbar.folder.name")), true);
        if attach(r, keys, t, || tips::folder(st)).clicked() {
            msgs.push(Msg::OpenFolder(snap.workdir.clone()));
        }
    });
}

/// A borderless group's width around buttons `slots` wide: the buttons and their spacing.
pub(super) fn group_width(slots: &[f32]) -> f32 {
    let n = slots.len() as f32;
    slots.iter().sum::<f32>() + widgets::TOOL_SPACING * (n - 1.0).max(0.0)
}

/// A tool button's width for its caption `label`; a split button's adds its caret.
pub(super) fn slot(ui: &egui::Ui, st: &AppState, label: &str, split: bool) -> f32 {
    widgets::tool_width(ui, label, st.settings.toolbar_labels) + if split { widgets::CARET_W } else { 0.0 }
}
