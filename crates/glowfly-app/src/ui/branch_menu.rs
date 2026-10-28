//! The right-click menu of a local branch, in the menu review's groups (spec §10): checkout (or,
//! for a branch another worktree has, open that worktree), push, push to another remote (only
//! when that offers more than Push) and fast-forward (only with an upstream); merge and rebase
//! (split rows whose eye button previews them), interactive rebase, checking for conflicts and
//! comparing with its version before the last rebase (Range-diff); a new branch or worktree,
//! rename and upstream; the stack items for a branch in a stack (`stack_menu.rs`); pull requests
//! (pushing the branch first when the code host lacks it, into the branch selected before;
//! without a code host, a disabled item saying why); hide or solo in the graph, and its colour;
//! copying its name or a link; the custom commands (`custom_menu.rs`); and delete (one item,
//! whose dialog offers Delete anyway for a branch with commits on no other branch).
//! The branch's name heads the menu once; other names in labels are cut to fit, in full on hover.
//! Disabled items say why on hover, with the same text `update` logs when it refuses a late
//! message.

use super::common::{menu_danger, menu_item, menu_item_if, menu_row, menu_row_preview};
use super::ref_visibility;
use super::theme::color;
use crate::conflict_check::ConflictCheckMsg;
use crate::custom_commands::Target;
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::OpPreviewMsg;
use crate::msg::RebaseMsg;
use crate::range_diff::RangeDiffMsg;
use crate::state::branches::{self, NO_REMOTES};
use crate::state::commit_menu::menu_trim;
use crate::state::custom_commands::MenuCommands;
use crate::state::gitflow_base::{Deleting, guard};
use crate::state::worktree_branches::{checkout_blocked, delete_blocked, elsewhere};
use crate::state::worktrees::open_blocked;
use crate::state::{ForgeState, Selection};
use crate::ui::widgets::menu::{item, item_named, named, separator, submenu};
use crate::ui::widgets::reason::ItemState;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, FontSelection, RichText, text::LayoutJob};
use glowfly_core::custom_commands::Place;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::settings::GraphFilter;
use glowfly_core::worktree_ops::WorktreeBase;

#[allow(clippy::too_many_arguments)]
pub fn local_branch_menu(
    ui: &mut egui::Ui,
    t: Lang,
    b: &BranchInfo,
    snap: &Snapshot,
    fs: Option<&ForgeState>,
    filter: &GraphFilter,
    sel: &Selection,
    cmds: &MenuCommands,
    msgs: &mut Vec<Msg>,
) {
    let name = b.name.as_str();
    super::stack_menu::header(ui, t, name, snap);
    primary_items(ui, t, b, snap, msgs);
    separator(ui);
    integrate_items(ui, t, b, snap, msgs);
    separator(ui);
    branch_items(ui, t, b, snap, msgs);
    super::stack_menu::items(ui, t, b, snap, fs, msgs);
    // Always drawn: without a code host, a disabled item says why there are no pull requests.
    separator(ui);
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
    separator(ui);
    ref_visibility::menu_items(ui, t, &format!("refs/heads/{name}"), filter, b.is_head, msgs);
    super::branch_colour_menu::items(ui, t, name, snap, msgs);
    separator(ui);
    menu_item(msgs, ui, t.tr("refs.copy.name_item"), Msg::Copy(name.to_owned()));
    let link = Msg::DeepLink(DeepLinkMsg::Copy(LinkTarget::LocalBranch(name.to_owned())));
    menu_item(msgs, ui, t.tr("refs.copy.link_item"), link);
    super::custom_menu::group(ui, t, cmds, Place::Branch, Target::Branch(name.to_owned()), msgs);
    if !b.is_head {
        separator(ui);
        delete_items(ui, t, b, snap, msgs);
    }
}

/// Checkout (or open the worktree that has it), Push, the "Push to" submenu (one entry per
/// remote, when that offers more than Push) and fast-forward (with an upstream).
fn primary_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    if !b.is_head {
        let checkout = Msg::Branch(BranchMsg::Checkout(name.to_owned()));
        menu_item_if(msgs, ui, t.tr("branch.menu.checkout"), checkout_blocked(snap, name).as_deref(), checkout);
        if let Some(wt) = elsewhere(snap, name) {
            let open = Msg::Worktree(WorktreeMsg::Open(wt.path.clone()));
            menu_item_if(msgs, ui, t.tr("branch.menu.open_worktree"), open_blocked(wt), open);
        }
    }
    push_items(ui, t, b, snap, msgs);
    if menu_trim::fast_forward_applies(b) {
        let ff_blocked = branches::fast_forward_blocked(b, snap.state);
        let ff = match branches::upstream_label(b) {
            Some(up) => item_named(named(t.tr("branch.menu.fast_forward_to"), &[("upstream", &up)])),
            None => item(t.tr("branch.menu.fast_forward")),
        };
        menu_row(msgs, ui, ff.blocked(ff_blocked.as_deref()), Msg::Branch(BranchMsg::FastForwardBranch(name.to_owned())));
    }
}

/// A new branch or worktree from it, rename and upstream.
fn branch_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
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

/// Merge (with its preview), rebase (with its preview), interactive rebase and conflict check
/// against the checked-out branch; on the checked-out branch itself, the conflict check against
/// its target. Then the compare with the branch before its last rebase.
fn integrate_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let current = snap.head_branch.as_deref().unwrap_or_default();
    if b.is_head {
        let check = Msg::ConflictCheck(ConflictCheckMsg::CheckNow);
        match glowfly_core::conflict_check::target_for(snap) {
            Some(target) => {
                menu_row(msgs, ui, item_named(named(t.tr("branch.menu.check_conflicts_with"), &[("branch", &target.name)])), check)
            }
            None => menu_item_if(msgs, ui, t.tr("branch.menu.check_conflicts"), Some(t.tr("conflict_check.no_target")), check),
        }
    } else if !current.is_empty() {
        let on = [("branch", current)];
        let preview = |op| Msg::OpPreview(OpPreviewMsg::Open(op));
        let merge = item_named(named(t.tr("branch.menu.merge_into"), &on));
        let rebase = item_named(named(t.tr("branch.menu.rebase_onto"), &on));
        match b.oid {
            Some(oid) => {
                let merge_preview = preview(PreviewOp::Merge { theirs: name.to_owned(), oid });
                menu_row_preview(
                    msgs,
                    ui,
                    merge,
                    (t.tr("op_preview.menu.merge"), None),
                    Msg::git("Merge", &["merge", name]),
                    merge_preview,
                );
                let rebase_preview = preview(PreviewOp::Rebase { onto: name.to_owned(), oid });
                menu_row_preview(
                    msgs,
                    ui,
                    rebase,
                    (t.tr("op_preview.menu.rebase"), None),
                    Msg::git("Rebase", &["rebase", name]),
                    rebase_preview,
                );
            }
            None => {
                menu_row(msgs, ui, merge, Msg::git("Merge", &["merge", name]));
                menu_row(msgs, ui, rebase, Msg::git("Rebase", &["rebase", name]));
            }
        }
        let onto = Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoBranch(name.to_owned())));
        menu_row(msgs, ui, item_named(named(t.tr("branch.menu.interactive_onto"), &on)), onto);
        let check = Msg::ConflictCheck(ConflictCheckMsg::CheckBranch(name.to_owned()));
        menu_row(msgs, ui, item_named(named(t.tr("branch.menu.check_conflicts_with"), &on)), check);
    }
    if b.oid.is_some() {
        let compare = Msg::RangeDiff(RangeDiffMsg::OpenRewrite(format!("refs/heads/{name}")));
        menu_item(msgs, ui, t.tr("branch.menu.range_diff"), compare);
    }
}

/// Push, and the "Push to" submenu (one entry per remote) when it offers more than Push.
fn push_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let has_remotes = !snap.remotes.is_empty();
    let push_blocked = branches::push_blocked(b, has_remotes);
    menu_item_if(
        msgs,
        ui,
        t.tr("branch.menu.push"),
        push_blocked.as_deref(),
        Msg::Branch(BranchMsg::PushBranch { branch: name.to_owned(), remote: None }),
    );
    if !menu_trim::push_to_adds_a_choice(b, snap) {
        return;
    }
    // egui draws the submenu arrow; the label has none of its own.
    submenu(ui, t.tr("branch.menu.push_to"), |ui| {
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

/// Delete (its dialog says what the branch would lose and offers Delete anyway when that is
/// anything) and, with an upstream on a remote, delete together with it (only when the
/// checked-out branch contains it). Both are disabled for a branch another worktree has; the
/// Git Flow production or development branch is asked about more firmly (`gitflow_base`).
fn delete_items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let head = snap.head_branch.as_deref();
    let in_worktree = delete_blocked(snap, name);
    let in_worktree = in_worktree.as_deref();
    let with_remote = branches::can_delete_with_remote(b);
    menu_danger(msgs, ui, t.tr("branch.menu.delete"), in_worktree, Msg::Branch(BranchMsg::DeleteDialog(name.to_owned())));
    if let Some(tr) = with_remote {
        let blocked = in_worktree.map(str::to_owned).or_else(|| branches::delete_with_remote_blocked(b, tr, head));
        let label = named(t.tr("branch.menu.delete_with_remote"), &[("remote_branch", &tr.display())]);
        let ask = guard(
            t,
            snap,
            name,
            Deleting::LocalAndRemote(&tr.remote),
            Msg::confirm_danger(
                t.tr("branch.delete_both.title"),
                branches::delete_with_remote_body(t, b, tr),
                t.tr("branch.delete_both.button"),
                Msg::Branch(BranchMsg::DeleteBranchAndRemote(name.to_owned())),
            ),
        );
        menu_row(msgs, ui, item_named(label).state(ItemState::danger_unless(blocked.as_deref())), ask);
    }
}
