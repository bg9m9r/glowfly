//! Pull request items shared by the branch row menus and the toolbar: create one (into the
//! branch selected before the menu opened, or from the checked-out branch into a remote one),
//! and push the branch first when the code host lacks it or some of its commits. A branch that
//! can't be pushed plainly gets a disabled item saying why; nothing is ever force-pushed.
//! Without a code host, a row menu keeps one disabled item saying why. Also the pull request
//! tooltip the sidebar and the dashboard share.

use super::common::{menu_item, menu_item_if};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::push_pr::{self, HostBranch};
use crate::state::{ForgeState, Selection};
use eframe::egui;
use git2::Oid;
use trellis_core::forge::PullRequest;
use trellis_core::repo::{BranchInfo, Snapshot};

/// A pull request row's tooltip (the sidebar's and the dashboard's): branches and author, the
/// reviewers' votes, the merge status, the checks and the start of the description.
pub(super) fn pr_tooltip(pr: &PullRequest) -> String {
    let mut tooltip = format!("{} into {}\nby {}", pr.source, pr.target, pr.author);
    for rv in &pr.reviewers {
        tooltip.push_str(&format!("\n{}: {}{}", rv.name, rv.state, if rv.required { " (required)" } else { "" }));
    }
    if let Some(ms) = &pr.merge_status {
        tooltip.push_str(&format!("\nmerge: {ms}"));
    }
    if let Some(c) = &pr.checks {
        tooltip.push_str(&format!("\n{}", c.summary()));
    }
    let d = pr.description.trim();
    if !d.is_empty() {
        let d: String = d.chars().take(300).collect();
        tooltip.push_str(&format!("\n\n{d}"));
    }
    tooltip
}

fn create(source: &str, target: Option<&str>) -> Msg {
    Msg::Forge(ForgeMsg::CreatePrDialog { source: source.to_owned(), target: target.map(str::to_owned) })
}

fn push(branch: &str, target: Option<&str>) -> Msg {
    Msg::Forge(ForgeMsg::PushAndStartPr { branch: branch.to_owned(), target: target.map(str::to_owned) })
}

/// A LOCAL row: open the branch's pull request, or create one, pushing first when needed.
pub(super) fn local(ui: &mut egui::Ui, b: &BranchInfo, snap: &Snapshot, fs: &ForgeState, sel: &Selection, msgs: &mut Vec<Msg>) {
    let (name, noun) = (b.name.as_str(), fs.pr_noun());
    if let Some(pr) = push_pr::open_pr(snap, fs, name) {
        menu_item(msgs, ui, format!("Open {noun} {}", fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone()));
        return;
    }
    let into = push_pr::chosen_target(sel, b, snap, fs);
    let into = into.as_deref();
    let state = push_pr::host_branch(b, snap, fs);
    // The service refuses a source it doesn't have, so a missing branch is only offered pushed.
    if !matches!(state, Some(HostBranch::Missing)) {
        menu_item(msgs, ui, format!("Create {noun}…"), create(name, None));
        if let (Some(t), false) = (into, push_pr::needs_push(&state)) {
            menu_item(msgs, ui, format!("Create {noun} into {t}…"), create(name, Some(t)));
        }
    }
    let label = format!("Push and create a {noun}…");
    match &state {
        Some(HostBranch::Missing | HostBranch::Ahead(_)) => {
            menu_item(msgs, ui, label, push(name, None));
            if let Some(t) = into {
                menu_item(msgs, ui, format!("Push and create a {noun} into {t}…"), push(name, Some(t)));
            }
        }
        Some(HostBranch::Blocked(why)) => menu_item_if(msgs, ui, label, Some(why.as_str()), push(name, None)),
        Some(HostBranch::Pushed) | None => {}
    }
}

/// A REMOTE row on the code host's remote whose branch has no open pull request: create one
/// from it (into the selected branch), or from the checked-out branch into it.
pub(super) fn remote(ui: &mut egui::Ui, name: &str, oid: Oid, snap: &Snapshot, fs: &ForgeState, sel: &Selection, msgs: &mut Vec<Msg>) {
    let noun = fs.pr_noun();
    menu_item(msgs, ui, format!("Create {noun}…"), create(name, None));
    if let Some(t) = push_pr::chosen_target_for(sel, name, Some(oid), None, snap, fs) {
        menu_item(msgs, ui, format!("Create {noun} into {t}…"), create(name, Some(&t)));
    }
    let Some(head) = snap.head_info() else { return };
    if push_pr::open_pr(snap, fs, &head.name).is_some() || push_pr::pr_source(snap, fs, &head.name) == name {
        return;
    }
    let h = head.name.as_str();
    match push_pr::host_branch(head, snap, fs) {
        state if push_pr::needs_push(&state) => {
            menu_item(msgs, ui, format!("Push {h} and create a {noun} into this branch…"), push(h, Some(name)));
        }
        Some(HostBranch::Blocked(why)) => {
            menu_item_if(msgs, ui, format!("Create {noun} from {h} into this branch…"), Some(why.as_str()), create(h, Some(name)));
        }
        _ => menu_item(msgs, ui, format!("Create {noun} from {h} into this branch…"), create(h, Some(name))),
    }
}

/// A LOCAL row when no remote is on a code host: one disabled item saying why.
pub(super) fn local_no_host(ui: &mut egui::Ui, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = push_pr::no_host_reason(push_pr::pr_remote(b, snap).as_deref());
    menu_item_if(msgs, ui, "Create pull request…", Some(&why), create(&b.name, None));
}

/// A REMOTE row when no remote is on a code host: one disabled item saying why, from the
/// checked-out branch into this one when that is another branch.
pub(super) fn remote_no_host(ui: &mut egui::Ui, remote: &str, name: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = push_pr::no_host_reason(Some(remote));
    match snap.head_info().filter(|h| !push_pr::is_same_branch(h, remote, name)) {
        Some(h) => {
            let label = format!("Create pull request from {} into this branch…", h.name);
            menu_item_if(msgs, ui, label, Some(&why), create(&h.name, Some(name)));
        }
        None => menu_item_if(msgs, ui, "Create pull request…", Some(&why), create(name, None)),
    }
}

/// The PR tool button's menu: Create (or Open, once the checked-out branch has an open pull
/// request), and Push and create.
pub(super) fn pr_menu(ui: &mut egui::Ui, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
    let head = snap.head_info();
    if let Some(pr) = head.and_then(|b| push_pr::open_pr(snap, fs, &b.name)) {
        menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone()));
        push_pr_item(ui, snap, fs, idle, msgs);
        return;
    }
    let r = ui.add_enabled(idle && head.is_some(), egui::Button::new(format!("Create {}…", fs.pr_noun())));
    if r.clicked()
        && let Some(b) = head
    {
        msgs.push(create(&b.name, None));
    }
    push_pr_item(ui, snap, fs, idle, msgs);
}

/// "Push <head> and create a …" in the Push and PR dropdowns: enabled when idle and the
/// checked-out branch has no open pull request and needs a push, otherwise saying why not.
pub(super) fn push_pr_item(ui: &mut egui::Ui, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
    let noun = fs.pr_noun();
    let item = push_pr::push_item(snap, fs);
    let label = match snap.head_info() {
        Some(b) => format!("Push {} and create a {noun}…", b.name),
        None => format!("Push and create a {noun}…"),
    };
    let mut r = ui.add_enabled(idle && item.is_ok(), egui::Button::new(label));
    if let Err(why) = &item {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked()
        && let Ok(branch) = item
    {
        msgs.push(push(&branch, None));
    }
}
