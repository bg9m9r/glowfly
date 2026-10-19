//! Pull request items shared by the branch row menus and the toolbar: create one (into the
//! branch selected before the menu opened, or from the checked-out branch into a remote one),
//! and push the branch first when the code host lacks it or some of its commits. A branch that
//! can't be pushed plainly gets a disabled item saying why; nothing is ever force-pushed.
//! Without a code host, a row menu keeps one disabled item saying why. Also the pull request
//! tooltip the sidebar and the dashboard share.

use super::common::{menu_item, menu_item_blocked, menu_item_if};
use super::widgets::reason::{self, ItemState};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::blocked::{BUSY, Blocked, short_form};
use crate::state::push_pr::{self, HostBranch};
use crate::state::{ForgeState, Selection};
use eframe::egui;
use git2::Oid;
use trellis_core::forge::PullRequest;
use trellis_core::i18n::Lang;
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
pub(super) fn local(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, fs: &ForgeState, sel: &Selection, msgs: &mut Vec<Msg>) {
    let (name, noun) = (b.name.as_str(), fs.pr_word(t));
    if let Some(pr) = push_pr::open_pr(snap, fs, name) {
        menu_item(msgs, ui, t.trf("pr.menu.open", &[("noun", &noun), ("pr", &fs.pr_label(pr.id))]), Msg::OpenUrl(pr.url.clone()));
        return;
    }
    let into = push_pr::chosen_target(sel, b, snap, fs);
    let into = into.as_deref();
    let state = push_pr::host_branch(b, snap, fs);
    // The service refuses a source it doesn't have, so a missing branch is only offered pushed.
    if !matches!(state, Some(HostBranch::Missing)) {
        menu_item(msgs, ui, t.trf("pr.menu.create", &[("noun", &noun)]), create(name, None));
        if let (Some(target), false) = (into, push_pr::needs_push(&state)) {
            menu_item(msgs, ui, t.trf("pr.menu.create_into", &[("noun", &noun), ("branch", &target)]), create(name, Some(target)));
        }
    }
    let label = t.trf("pr.menu.push_create", &[("noun", &noun)]);
    match &state {
        Some(HostBranch::Missing | HostBranch::Ahead(_)) => {
            menu_item(msgs, ui, label, push(name, None));
            if let Some(target) = into {
                let label = t.trf("pr.menu.push_create_into", &[("noun", &noun), ("branch", &target)]);
                menu_item(msgs, ui, label, push(name, Some(target)));
            }
        }
        Some(HostBranch::Blocked(why)) => menu_item_if(msgs, ui, label, Some(why.as_str()), push(name, None)),
        Some(HostBranch::Pushed) | None => {}
    }
}

/// A REMOTE row on the code host's remote whose branch has no open pull request: create one
/// from it (into the selected branch), or from the checked-out branch into it.
pub(super) fn remote(
    ui: &mut egui::Ui,
    t: Lang,
    (name, oid): (&str, Oid),
    snap: &Snapshot,
    fs: &ForgeState,
    sel: &Selection,
    msgs: &mut Vec<Msg>,
) {
    let noun = fs.pr_word(t);
    menu_item(msgs, ui, t.trf("pr.menu.create", &[("noun", &noun)]), create(name, None));
    if let Some(target) = push_pr::chosen_target_for(sel, name, Some(oid), None, snap, fs) {
        menu_item(msgs, ui, t.trf("pr.menu.create_into", &[("noun", &noun), ("branch", &target)]), create(name, Some(&target)));
    }
    let Some(head) = snap.head_info() else { return };
    if push_pr::open_pr(snap, fs, &head.name).is_some() || push_pr::pr_source(snap, fs, &head.name) == name {
        return;
    }
    let h = head.name.as_str();
    let args: [(&str, &dyn std::fmt::Display); 2] = [("noun", &noun), ("branch", &h)];
    match push_pr::host_branch(head, snap, fs) {
        state if push_pr::needs_push(&state) => {
            menu_item(msgs, ui, t.trf("pr.menu.push_head_into_this", &args), push(h, Some(name)));
        }
        Some(HostBranch::Blocked(why)) => {
            menu_item_if(msgs, ui, t.trf("pr.menu.create_from_into_this", &args), Some(why.as_str()), create(h, Some(name)));
        }
        _ => menu_item(msgs, ui, t.trf("pr.menu.create_from_into_this", &args), create(h, Some(name))),
    }
}

/// Why a pull request can't go through `remote` (or no remote at all), with a short form that
/// names the remote.
fn no_host(remote: Option<&str>) -> Blocked<'static> {
    let full = push_pr::no_host_reason(remote);
    let short = match remote {
        Some(r) => format!("{r} isn't on a code host"),
        None => short_form(&full).into_owned(),
    };
    Blocked::new(short, full)
}

/// A LOCAL row when no remote is on a code host: one disabled item saying why.
pub(super) fn local_no_host(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = no_host(push_pr::pr_remote(b, snap).as_deref());
    let label = t.trf("pr.menu.create", &[("noun", &t.tr("forge.noun.pull_request"))]);
    menu_item_blocked(msgs, ui, label, Some(why), create(&b.name, None));
}

/// A REMOTE row when no remote is on a code host: one disabled item saying why, from the
/// checked-out branch into this one when that is another branch.
pub(super) fn remote_no_host(ui: &mut egui::Ui, t: Lang, remote: &str, name: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = no_host(Some(remote));
    let noun = t.tr("forge.noun.pull_request");
    match snap.head_info().filter(|h| !push_pr::is_same_branch(h, remote, name)) {
        Some(h) => {
            let label = t.trf("pr.menu.create_from_into_this", &[("noun", &noun), ("branch", &h.name)]);
            menu_item_blocked(msgs, ui, label, Some(why), create(&h.name, Some(name)));
        }
        None => menu_item_blocked(msgs, ui, t.trf("pr.menu.create", &[("noun", &noun)]), Some(why), create(name, None)),
    }
}

/// The PR tool button's menu: Create (or Open, once the checked-out branch has an open pull
/// request), and Push and create.
pub(super) fn pr_menu(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
    let head = snap.head_info();
    let noun = fs.pr_word(t);
    if let Some(pr) = head.and_then(|b| push_pr::open_pr(snap, fs, &b.name)) {
        menu_item(msgs, ui, t.trf("pr.menu.open", &[("noun", &noun), ("pr", &fs.pr_label(pr.id))]), Msg::OpenUrl(pr.url.clone()));
        push_pr_item(ui, t, snap, fs, idle, msgs);
        return;
    }
    let no_head = t.trf("pr.menu.no_head", &[("noun", &noun)]);
    let why = if !idle { Some(BUSY) } else { head.is_none().then_some(no_head.as_str()) };
    let r = reason::menu_button(ui, t.trf("pr.menu.create", &[("noun", &noun)]), "", ItemState::blocked_by(why));
    if r.clicked()
        && let Some(b) = head
    {
        msgs.push(create(&b.name, None));
    }
    push_pr_item(ui, t, snap, fs, idle, msgs);
}

/// "Push <head> and create a …" in the Push and PR dropdowns: enabled when idle and the
/// checked-out branch has no open pull request and needs a push, otherwise saying why not.
pub(super) fn push_pr_item(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
    let noun = fs.pr_word(t);
    let item = push_pr::push_item(snap, fs);
    let label = match snap.head_info() {
        Some(b) => t.trf("pr.menu.push_head_create", &[("noun", &noun), ("branch", &b.name)]),
        None => t.trf("pr.menu.push_create", &[("noun", &noun)]),
    };
    let why = if !idle { Some(BUSY) } else { item.as_ref().err().map(String::as_str) };
    let r = reason::menu_button(ui, label, "", ItemState::blocked_by(why));
    if r.clicked()
        && let Ok(branch) = item
    {
        msgs.push(push(&branch, None));
    }
}
