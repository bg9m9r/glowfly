//! The right-click menu of a local branch, in groups: checkout (or, for a branch another
//! worktree has, open that worktree), a new branch or worktree, rename and upstream; merge,
//! rebase, checking for conflicts, fast-forward and push; pull requests (pushing the branch
//! first when the code host lacks it, into the branch selected before; without a code host, a
//! disabled item saying why); hide or solo in the graph, and its colour; the Copy submenu; and
//! delete. The branch's name heads the menu once; the items leave it out. Disabled items say why
//! on hover, with the same text `update` logs when it refuses a late message.

use super::common::{menu_header, menu_item, menu_item_if};
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
use trellis_core::i18n::Lang;
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{BranchInfo, Snapshot};
use trellis_core::settings::GraphFilter;
use trellis_core::worktree_ops::WorktreeBase;

#[allow(clippy::too_many_arguments)]
pub fn local_branch_menu(
    ui: &mut egui::Ui,
    t: Lang,
    b: &BranchInfo,
    snap: &Snapshot,
    fs: Option<&ForgeState>,
    filter: &GraphFilter,
    sel: &Selection,
    msgs: &mut Vec<Msg>,
) {
    let name = b.name.as_str();
    menu_header(ui, name);
    branch_items(ui, t, b, snap, msgs);
    ui.separator();
    integrate_items(ui, t, b, snap, msgs);
    push_items(ui, t, b, snap, msgs);
    // Always drawn: without a code host, a disabled item says why there are no pull requests.
    ui.separator();
    match fs.filter(|fs| fs.host.is_some()) {
        Some(fs) => super::pr_menu_items::local(ui, t, b, snap, fs, sel, msgs),
        None => super::pr_menu_items::local_no_host(ui, t, b, snap, msgs),
    }
    if let Some(fs) = fs
        && let Some(key) = fs.branch_issue(name)
    {
        let label = t.trf("branch.menu.open_issue", &[("noun", &fs.issue_word(t)), ("issue", &fs.mention(&key))]);
        menu_item(msgs, ui, label, Msg::OpenUrl(fs.issue_url(&key)));
    }
    ui.separator();
    ref_visibility::menu_items(ui, t, &format!("refs/heads/{name}"), filter, b.is_head, msgs);
    super::branch_colour_menu::items(ui, t, name, snap, msgs);
    ui.separator();
    ui.menu_button(t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("refs.copy.name"), Msg::Copy(name.to_owned()));
        menu_item(msgs, ui, t.tr("refs.copy.link"), Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::LocalBranch(name.to_owned()))));
    });
    if !b.is_head {
        ui.separator();
        delete_items(ui, t, b, snap, msgs);
    }
}

/// Checkout (or open the worktree that has it), a new branch or worktree from it, rename and
/// upstream.
fn branch_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    if !b.is_head {
        let checkout = Msg::Branch(BranchMsg::Checkout(name.to_owned()));
        menu_item_if(msgs, ui, t.tr("branch.menu.checkout"), checkout_blocked(snap, name).as_deref(), checkout);
        if let Some(wt) = elsewhere(snap, name) {
            let open = Msg::Worktree(WorktreeMsg::Open(wt.path.clone()));
            menu_item_if(msgs, ui, t.tr("branch.menu.open_worktree"), open_blocked(wt), open);
        }
    }
    if let Some(o) = b.oid {
        let new_branch = Msg::Branch(BranchMsg::NewBranch { start: Some(o.to_string()), name: String::new() });
        menu_item(msgs, ui, t.tr("commits.menu.create_branch"), new_branch);
        let base = WorktreeBase::Branch(name.to_owned());
        menu_item(msgs, ui, t.tr("branch.menu.create_worktree"), Msg::Worktree(WorktreeMsg::CreateDialog(base)));
    }
    menu_item(msgs, ui, t.tr("branch.menu.rename"), Msg::Branch(BranchMsg::RenameBranchDialog(name.to_owned())));
    let no_remotes = snap.remotes.is_empty().then_some(NO_REMOTES);
    menu_item_if(msgs, ui, t.tr("branch.menu.set_upstream"), no_remotes, Msg::Branch(BranchMsg::SetUpstreamDialog(name.to_owned())));
    if b.tracking.is_some() {
        menu_item(msgs, ui, t.tr("branch.menu.unset_upstream"), Msg::Branch(BranchMsg::UnsetUpstream(name.to_owned())));
    }
}

/// Merge, rebase and conflict check against the checked-out branch; on the checked-out branch
/// itself, the conflict check against its target.
fn integrate_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let current = snap.head_branch.as_deref().unwrap_or_default();
    if b.is_head {
        let check = Msg::ConflictCheck(ConflictCheckMsg::CheckNow);
        match trellis_core::conflict_check::target_for(snap) {
            Some(target) => menu_item(msgs, ui, t.trf("branch.menu.check_conflicts_with", &[("branch", &target.name)]), check),
            None => menu_item_if(msgs, ui, t.tr("branch.menu.check_conflicts"), Some(NO_TARGET), check),
        }
    } else if !current.is_empty() {
        let args: [(&str, &dyn std::fmt::Display); 1] = [("branch", &current)];
        menu_item(msgs, ui, t.trf("branch.menu.merge_into", &args), Msg::git("Merge", &["merge", name]));
        menu_item(msgs, ui, t.trf("branch.menu.rebase_onto", &args), Msg::git("Rebase", &["rebase", name]));
        let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoBranch(name.to_owned())));
        menu_item(msgs, ui, t.trf("branch.menu.interactive_onto", &args), onto);
        let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(name.to_owned()));
        menu_item(msgs, ui, t.trf("branch.menu.check_conflicts_with", &args), check);
    }
}

/// Fast-forward, Push, and the "Push to" submenu (one entry per remote).
fn push_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let ff_label = match branches::upstream_label(b) {
        Some(up) => t.trf("branch.menu.fast_forward_to", &[("upstream", &up)]),
        None => t.tr("branch.menu.fast_forward").to_owned(),
    };
    let ff_blocked = branches::fast_forward_blocked(b, snap.state);
    menu_item_if(msgs, ui, ff_label, ff_blocked.as_deref(), Msg::Branch(BranchMsg::FastForwardBranch(name.to_owned())));

    let has_remotes = !snap.remotes.is_empty();
    let push_blocked = branches::push_blocked(b, has_remotes);
    menu_item_if(
        msgs,
        ui,
        t.tr("branch.menu.push"),
        push_blocked.as_deref(),
        Msg::Branch(BranchMsg::PushBranch { branch: name.to_owned(), remote: None }),
    );
    if !has_remotes {
        return;
    }
    // egui draws the submenu arrow; the label has none of its own.
    ui.menu_button(t.tr("branch.menu.push_to"), |ui| {
        let tracked = branches::remote_tracking(b).map(|tr| tr.remote.as_str());
        for (remote, _) in &snap.remotes {
            let is_upstream = tracked == Some(remote.as_str());
            let mut job = LayoutJob::default();
            RichText::new(remote).append_to(&mut job, ui.style(), FontSelection::Default, egui::Align::Center);
            if is_upstream {
                RichText::new(format!(" {}", t.tr("branch.menu.push_to.upstream"))).color(color::weak()).append_to(
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
fn delete_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let head = snap.head_branch.as_deref();
    let in_worktree = delete_blocked(snap, name);
    let in_worktree = in_worktree.as_deref();
    menu_item_if(
        msgs,
        ui,
        t.tr("branch.menu.delete"),
        in_worktree,
        Msg::confirm_danger(
            t.tr("branch.delete.title"),
            t.trf("branch.delete.body", &[("branch", &name)]),
            t.tr("common.delete.button"),
            Msg::git("Delete branch", &["branch", "-d", name]),
        ),
    );
    menu_item_if(
        msgs,
        ui,
        t.tr("branch.menu.force_delete"),
        in_worktree,
        Msg::confirm_danger(
            t.tr("branch.force_delete.title"),
            t.trf("branch.force_delete.body", &[("branch", &name)]),
            t.tr("common.delete.button"),
            Msg::git("Delete branch", &["branch", "-D", name]),
        ),
    );
    if let Some(tr) = branches::can_delete_with_remote(b) {
        let blocked = in_worktree.map(str::to_owned).or_else(|| branches::delete_with_remote_blocked(b, tr, head));
        menu_item_if(
            msgs,
            ui,
            t.trf("branch.menu.delete_with_remote", &[("remote_branch", &tr.display())]),
            blocked.as_deref(),
            Msg::confirm_danger(
                t.tr("branch.delete_both.title"),
                branches::delete_with_remote_body(t, b, tr),
                t.tr("branch.delete_both.button"),
                Msg::Branch(BranchMsg::DeleteBranchAndRemote(name.to_owned())),
            ),
        );
    }
}
