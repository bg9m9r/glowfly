//! The right-click menu of a local branch: checkout, merge and rebase, fast-forward, push,
//! pull requests, rename, upstream, hide or solo in the graph, and delete. Disabled items say why on hover, with the same
//! text `update` logs when it refuses a late message.

use super::common::{menu_item, menu_item_if};
use super::ref_visibility;
use super::theme::color;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::state::ForgeState;
use crate::state::branches::{self, NO_REMOTES};
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
    msgs: &mut Vec<Msg>,
) {
    let name = b.name.as_str();
    let current = snap.head_branch.as_deref().unwrap_or_default();
    if !b.is_head {
        menu_item(
            msgs,
            ui,
            format!("Checkout {name}"),
            Msg::Git { label: "Checkout".into(), cmds: vec![crate::state::branches::checkout_local(name)] },
        );
        if !current.is_empty() {
            menu_item(msgs, ui, format!("Merge {name} into {current}"), Msg::git("Merge", &["merge", name]));
            menu_item(msgs, ui, format!("Rebase {current} onto {name}"), Msg::git("Rebase", &["rebase", name]));
            let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoBranch(name.to_owned())));
            menu_item(msgs, ui, format!("Interactive rebase {current} onto {name}…"), onto);
        }
        ui.separator();
    }
    push_items(ui, b, snap, msgs);
    if let Some(fs) = fs {
        ui.separator();
        if fs.host.is_some() {
            match fs.pr_for_branch(name) {
                Some(pr) => menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone())),
                None => menu_item(msgs, ui, format!("Create {}…", fs.pr_noun()), Msg::CreatePrDialog(name.to_owned())),
            }
        }
        if let Some(key) = fs.branch_issue(name) {
            menu_item(msgs, ui, format!("Open {} {}", fs.issue_noun(), fs.mention(&key)), Msg::OpenUrl(fs.issue_url(&key)));
        }
    }
    ui.separator();
    if let Some(o) = b.oid {
        menu_item(msgs, ui, "Create branch here…", Msg::NewBranch { start: Some(o.to_string()), name: String::new() });
        menu_item(
            msgs,
            ui,
            format!("Create worktree from {name}…"),
            Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Branch(name.to_owned()))),
        );
    }
    menu_item(msgs, ui, format!("Rename {name}…"), Msg::RenameBranchDialog(name.to_owned()));
    let no_remotes = snap.remotes.is_empty().then_some(NO_REMOTES);
    menu_item_if(msgs, ui, "Set upstream…", no_remotes, Msg::SetUpstreamDialog(name.to_owned()));
    if b.tracking.is_some() {
        menu_item(msgs, ui, "Unset upstream", Msg::UnsetUpstream(name.to_owned()));
    }
    menu_item(msgs, ui, "Copy branch name", Msg::Copy(name.to_owned()));
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/heads/{name}"), name, filter, b.is_head, msgs);
    if !b.is_head {
        ui.separator();
        delete_items(ui, b, snap.head_branch.as_deref(), msgs);
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
    menu_item_if(msgs, ui, ff_label, ff_blocked.as_deref(), Msg::FastForwardBranch(name.to_owned()));

    let has_remotes = !snap.remotes.is_empty();
    let push_blocked = branches::push_blocked(b, has_remotes);
    menu_item_if(msgs, ui, format!("Push {name}"), push_blocked.as_deref(), Msg::PushBranch { branch: name.to_owned(), remote: None });
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
            menu_item_if(msgs, ui, job, blocked, Msg::PushBranch { branch: name.to_owned(), remote: Some(remote.clone()) });
        }
    });
}

/// Delete, force delete and, with an upstream on a remote, delete together with it (only when
/// `head`, the checked-out branch, contains it).
fn delete_items(ui: &mut egui::Ui, b: &BranchInfo, head: Option<&str>, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    menu_item(
        msgs,
        ui,
        format!("Delete {name}"),
        Msg::confirm("Delete branch", format!("Delete local branch {name}?"), "Delete", Msg::git("Delete branch", &["branch", "-d", name])),
    );
    menu_item(
        msgs,
        ui,
        format!("Force delete {name}"),
        Msg::confirm(
            "Force delete branch",
            format!("Delete {name} even if it has unmerged commits?"),
            "Delete",
            Msg::git("Delete branch", &["branch", "-D", name]),
        ),
    );
    if let Some(t) = branches::can_delete_with_remote(b) {
        let blocked = branches::delete_with_remote_blocked(b, t, head);
        menu_item_if(
            msgs,
            ui,
            format!("Delete {name} and {}", t.display()),
            blocked.as_deref(),
            Msg::confirm(
                "Delete local and remote branch",
                branches::delete_with_remote_body(b, t),
                "Delete both",
                Msg::DeleteBranchAndRemote(name.to_owned()),
            ),
        );
    }
}
