//! The right-click menu of a local branch: checkout (or, for a branch another worktree has, open
//! that worktree), merge and rebase, checking for conflicts,
//! fast-forward, push, pull requests (pushing the branch first when the code host lacks it,
//! into the branch selected before), rename, upstream, hide or solo in the graph, and delete.
//! Disabled items say why on hover, with the same text `update` logs when it refuses a late
//! message.

use super::common::{menu_item, menu_item_if};
use super::ref_visibility;
use super::theme::color;
use crate::conflict_check::ConflictCheckMsg;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::state::branches::{self, NO_REMOTES};
use crate::state::conflict_check::NO_TARGET;
use crate::state::worktree_branches::{checkout_blocked, delete_blocked, elsewhere};
use crate::state::worktrees::open_blocked;
use crate::state::{ForgeState, Selection};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, FontSelection, RichText, text::LayoutJob};
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{BranchInfo, Snapshot};
use trellis_core::settings::GraphFilter;
use trellis_core::worktree_ops::WorktreeBase;

pub fn local_branch_menu(
    ui: &mut egui::Ui,
    b: &BranchInfo,
    snap: &Snapshot,
    fs: Option<&ForgeState>,
    filter: &GraphFilter,
    sel: &Selection,
    msgs: &mut Vec<Msg>,
) {
    let name = b.name.as_str();
    let current = snap.head_branch.as_deref().unwrap_or_default();
    if !b.is_head {
        let checkout = Msg::Branch(BranchMsg::Checkout(name.to_owned()));
        menu_item_if(msgs, ui, format!("Checkout {name}"), checkout_blocked(snap, name).as_deref(), checkout);
        if let Some(wt) = elsewhere(snap, name) {
            let open = Msg::Worktree(WorktreeMsg::Open(wt.path.clone()));
            menu_item_if(msgs, ui, format!("Open worktree {}", wt.folder_name()), open_blocked(wt), open);
        }
        if !current.is_empty() {
            menu_item(msgs, ui, format!("Merge {name} into {current}"), Msg::git("Merge", &["merge", name]));
            menu_item(msgs, ui, format!("Rebase {current} onto {name}"), Msg::git("Rebase", &["rebase", name]));
            let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoBranch(name.to_owned())));
            menu_item(msgs, ui, format!("Interactive rebase {current} onto {name}…"), onto);
            let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(name.to_owned()));
            menu_item(msgs, ui, format!("Check for conflicts with {current}"), check);
        }
        ui.separator();
    } else {
        let check = Msg::ConflictCheck(ConflictCheckMsg::CheckNow);
        match trellis_core::conflict_check::target_for(snap) {
            Some(t) => menu_item(msgs, ui, format!("Check for conflicts with {}", t.name), check),
            None => menu_item_if(msgs, ui, "Check for conflicts", Some(NO_TARGET), check),
        }
        ui.separator();
    }
    push_items(ui, b, snap, msgs);
    if let Some(fs) = fs {
        ui.separator();
        if fs.host.is_some() {
            super::pr_menu_items::local(ui, b, snap, fs, sel, msgs);
        }
        if let Some(key) = fs.branch_issue(name) {
            menu_item(msgs, ui, format!("Open {} {}", fs.issue_noun(), fs.mention(&key)), Msg::OpenUrl(fs.issue_url(&key)));
        }
    }
    ui.separator();
    if let Some(o) = b.oid {
        menu_item(msgs, ui, "Create branch here…", Msg::Branch(BranchMsg::NewBranch { start: Some(o.to_string()), name: String::new() }));
        menu_item(
            msgs,
            ui,
            format!("Create worktree from {name}…"),
            Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Branch(name.to_owned()))),
        );
    }
    menu_item(msgs, ui, format!("Rename {name}…"), Msg::Branch(BranchMsg::RenameBranchDialog(name.to_owned())));
    let no_remotes = snap.remotes.is_empty().then_some(NO_REMOTES);
    menu_item_if(msgs, ui, "Set upstream…", no_remotes, Msg::Branch(BranchMsg::SetUpstreamDialog(name.to_owned())));
    if b.tracking.is_some() {
        menu_item(msgs, ui, "Unset upstream", Msg::Branch(BranchMsg::UnsetUpstream(name.to_owned())));
    }
    menu_item(msgs, ui, "Copy branch name", Msg::Copy(name.to_owned()));
    let link = Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::LocalBranch(name.to_owned())));
    menu_item(msgs, ui, format!("Copy link to branch {name}"), link);
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/heads/{name}"), name, filter, b.is_head, msgs);
    if !b.is_head {
        ui.separator();
        delete_items(ui, b, snap, msgs);
    }
}

/// Fast-forward, Push, and the "Push to" submenu (one entry per remote).
fn push_items(ui: &mut egui::Ui, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let ff_label = match branches::upstream_label(b) {
        Some(up) => format!("Fast-forward {name} to {up}"),
        None => format!("Fast-forward {name}"),
    };
    let ff_blocked = branches::fast_forward_blocked(b, snap.state);
    menu_item_if(msgs, ui, ff_label, ff_blocked.as_deref(), Msg::Branch(BranchMsg::FastForwardBranch(name.to_owned())));

    let has_remotes = !snap.remotes.is_empty();
    let push_blocked = branches::push_blocked(b, has_remotes);
    menu_item_if(
        msgs,
        ui,
        format!("Push {name}"),
        push_blocked.as_deref(),
        Msg::Branch(BranchMsg::PushBranch { branch: name.to_owned(), remote: None }),
    );
    if !has_remotes {
        return;
    }
    // egui draws the submenu arrow; the label has none of its own.
    ui.menu_button(format!("Push {name} to"), |ui| {
        let tracked = branches::remote_tracking(b).map(|t| t.remote.as_str());
        for (remote, _) in &snap.remotes {
            let is_upstream = tracked == Some(remote.as_str());
            let mut job = LayoutJob::default();
            RichText::new(remote).append_to(&mut job, ui.style(), FontSelection::Default, egui::Align::Center);
            if is_upstream {
                RichText::new(" (upstream)").color(color::weak()).append_to(
                    &mut job,
                    ui.style(),
                    FontSelection::Default,
                    egui::Align::Center,
                );
            }
            // Only the upstream's state is known; other remotes are left to git to accept or reject.
            let blocked = if is_upstream { push_blocked.as_deref() } else { None };
            menu_item_if(
                msgs,
                ui,
                job,
                blocked,
                Msg::Branch(BranchMsg::PushBranch { branch: name.to_owned(), remote: Some(remote.clone()) }),
            );
        }
    });
}

/// Delete, force delete and, with an upstream on a remote, delete together with it (only when
/// the checked-out branch contains it). All are disabled for a branch another worktree has.
fn delete_items(ui: &mut egui::Ui, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let head = snap.head_branch.as_deref();
    let in_worktree = delete_blocked(snap, name);
    let in_worktree = in_worktree.as_deref();
    menu_item_if(
        msgs,
        ui,
        format!("Delete {name}"),
        in_worktree,
        Msg::confirm("Delete branch", format!("Delete local branch {name}?"), "Delete", Msg::git("Delete branch", &["branch", "-d", name])),
    );
    menu_item_if(
        msgs,
        ui,
        format!("Force delete {name}"),
        in_worktree,
        Msg::confirm(
            "Force delete branch",
            format!("Delete {name} even if it has unmerged commits?"),
            "Delete",
            Msg::git("Delete branch", &["branch", "-D", name]),
        ),
    );
    if let Some(t) = branches::can_delete_with_remote(b) {
        let blocked = in_worktree.map(str::to_owned).or_else(|| branches::delete_with_remote_blocked(b, t, head));
        menu_item_if(
            msgs,
            ui,
            format!("Delete {name} and {}", t.display()),
            blocked.as_deref(),
            Msg::confirm(
                "Delete local and remote branch",
                branches::delete_with_remote_body(b, t),
                "Delete both",
                Msg::Branch(BranchMsg::DeleteBranchAndRemote(name.to_owned())),
            ),
        );
    }
}
