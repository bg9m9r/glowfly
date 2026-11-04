//! Pull request items shared by the branch row menus and the toolbar: create one (into the
//! branch selected before the menu opened, or from the checked-out branch into a remote one),
//! and push the branch first when the code host lacks it or some of its commits. A branch that
//! can't be pushed plainly gets a disabled item saying why; nothing is ever force-pushed.
//! Without a code host, a row menu keeps one disabled item saying why. Also the pull request
//! tooltip the sidebar and the dashboard share, and the PULL REQUESTS row's checkout item.

use crate::common::{menu_item, menu_item_if, menu_item_tip};
use crate::widgets::menu;
use crate::widgets::reason::{self, ItemState};
use eframe::egui;
use git2::Oid;
use glowfly_core::forge::PullRequest;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_model::msg::Msg;
use glowfly_model::msg::{BranchMsg, ForgeMsg};
use glowfly_model::state::blocked::BUSY;
use glowfly_model::state::push_pr::{self, HostBranch};
use glowfly_model::state::{ForgeState, Selection};

/// The PULL REQUESTS row's checkout of the source branch: its local branch like any other (Open
/// <source> in its worktree when another worktree has it, Prune and check out when that
/// worktree's folder is missing); otherwise a new branch tracking the remote one. Disabled with
/// the reason for a name git refuses or a branch this repository lacks.
pub(crate) fn checkout_item(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, f: &ForgeState, pr: &PullRequest, msgs: &mut Vec<Msg>) {
    use crate::widgets::menu::named;
    use glowfly_model::state::worktree_branches::{Elsewhere, checkout_elsewhere};
    let m = BranchMsg::CheckoutPullRequest { id: pr.id };
    let blocked = glowfly_model::state::branches::pr_checkout(snap, &f.remote, &pr.source).err();
    let local = snap.locals.iter().any(|b| b.name == pr.source);
    let elsewhere = (local && blocked.is_none()).then(|| checkout_elsewhere(t, snap, &pr.source)).flatten();
    let source = [("branch", pr.source.as_str())];
    match elsewhere {
        Some(Elsewhere::Open { tip }) => {
            let label = named(t.tr("remote_branch.menu.open_in_worktree"), &source).full;
            menu_item_tip(msgs, ui, label, &tip, Msg::Branch(m));
        }
        Some(Elsewhere::Prune { tip }) => {
            let label = named(t.tr("remote_branch.menu.prune_checkout"), &source).full;
            menu_item_tip(msgs, ui, label, &tip, Msg::Branch(BranchMsg::PruneThen(Box::new(m))));
        }
        None => menu_item_if(msgs, ui, format!("Checkout {}", pr.source), blocked.as_deref(), Msg::Branch(m)),
    }
}

/// A pull request row's tooltip (the sidebar's and the dashboard's): branches and author, the
/// reviewers' votes, the merge status, the checks and the start of the description.
pub(crate) fn pr_tooltip(pr: &PullRequest) -> String {
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
pub(crate) fn local(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, fs: &ForgeState, sel: &Selection, msgs: &mut Vec<Msg>) {
    let (name, noun) = (b.name.as_str(), fs.pr_word(t));
    if let Some(pr) = push_pr::open_pr(snap, fs, name) {
        menu_item(msgs, ui, t.trf("pr.menu.open", &[("noun", &noun), ("pr", &fs.pr_label(pr.id))]), Msg::OpenUrl(pr.url.clone()));
        return;
    }
    let into = push_pr::chosen_target(sel, b, snap, fs);
    let into = into.as_deref();
    let state = push_pr::host_branch(t, b, snap, fs);
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
pub(crate) fn remote(
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
    match push_pr::host_branch(t, head, snap, fs) {
        state if push_pr::needs_push(&state) => {
            menu_item(msgs, ui, t.trf("pr.menu.push_head_into_this", &args), push(h, Some(name)));
        }
        Some(HostBranch::Blocked(why)) => {
            menu_item_if(msgs, ui, t.trf("pr.menu.create_from_into_this", &args), Some(why.as_str()), create(h, Some(name)));
        }
        _ => menu_item(msgs, ui, t.trf("pr.menu.create_from_into_this", &args), create(h, Some(name))),
    }
}

/// A LOCAL row when no remote is on a code host: one disabled item saying why.
pub(crate) fn local_no_host(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = push_pr::no_host_reason(push_pr::pr_remote(b, snap).as_deref());
    let label = t.trf("pr.menu.create", &[("noun", &t.tr("forge.noun.pull_request"))]);
    menu_item_if(msgs, ui, label, Some(&why), create(&b.name, None));
}

/// A REMOTE row when no remote is on a code host: one disabled item saying why, from the
/// checked-out branch into this one when that is another branch.
pub(crate) fn remote_no_host(ui: &mut egui::Ui, t: Lang, remote: &str, name: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let why = push_pr::no_host_reason(Some(remote));
    let noun = t.tr("forge.noun.pull_request");
    match snap.head_info().filter(|h| !push_pr::is_same_branch(h, remote, name)) {
        Some(h) => {
            let label = t.trf("pr.menu.create_from_into_this", &[("noun", &noun), ("branch", &h.name)]);
            menu_item_if(msgs, ui, label, Some(&why), create(&h.name, Some(name)));
        }
        None => menu_item_if(msgs, ui, t.trf("pr.menu.create", &[("noun", &noun)]), Some(&why), create(name, None)),
    }
}

/// The PR tool button's menu: Create (or Open, once the checked-out branch has an open pull
/// request), and Push and create.
pub(crate) fn pr_menu(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
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

/// Whether the PR dropdown holds an item that can be used now: opening the open pull request,
/// creating one for the checked-out branch, or pushing it and creating one.
pub(crate) fn pr_menu_live(t: Lang, snap: &Snapshot, fs: &ForgeState, idle: bool) -> bool {
    let head = snap.head_info();
    head.and_then(|b| push_pr::open_pr(snap, fs, &b.name)).is_some()
        || (idle && (head.is_some() || push_pr::push_item(t, snap, fs).is_ok()))
}

/// "Push <head> and create a …" in the Push and PR dropdowns: enabled when idle and the
/// checked-out branch has no open pull request and needs a push, otherwise saying why not.
pub(crate) fn push_pr_item(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, fs: &ForgeState, idle: bool, msgs: &mut Vec<Msg>) {
    let noun = fs.pr_word(t);
    let item = push_pr::push_item(t, snap, fs);
    // A long branch name is cut in its middle; the whole label is its tooltip.
    let label = match snap.head_info() {
        Some(b) => menu::named(t.tr("pr.menu.push_head_create"), &[("noun", noun), ("branch", &b.name)]),
        None => menu::named(t.tr("pr.menu.push_create"), &[("noun", noun)]),
    };
    let why = if !idle { Some(BUSY) } else { item.as_ref().err().map(String::as_str) };
    let r = menu::item_named(label).state(ItemState::blocked_by(why)).show(ui);
    if r.clicked()
        && let Ok(branch) = item
    {
        msgs.push(push(&branch, None));
    }
}
