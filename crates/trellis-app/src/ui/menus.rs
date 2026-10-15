//! Right-click menus for commits and remote branches (local branches: `branch_menu.rs`).

use super::common::{menu_item, short};
use super::ref_visibility;
use super::theme::color;
use crate::msg::BranchMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::SelectionMsg;
use crate::state::ForgeState;
use crate::state::selection::MultiSelection;
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, RichText};
use git2::Oid;
use trellis_core::rebase::RebaseBase;
use trellis_core::settings::GraphFilter;
use trellis_core::worktree_ops::WorktreeBase;

pub fn commit_menu(ui: &mut egui::Ui, oid: Oid, summary: &str, msgs: &mut Vec<Msg>) {
    let sha = oid.to_string();
    let s = short(oid);
    menu_item(msgs, ui, "Create branch here…", Msg::Branch(BranchMsg::NewBranch { start: Some(sha.clone()), name: String::new() }));
    menu_item(msgs, ui, "Create tag here…", Msg::Tag(TagMsg::NewTag { at: sha.clone(), annotated: false }));
    menu_item(msgs, ui, "Create worktree from this commit…", Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(oid))));
    menu_item(msgs, ui, "Checkout this commit (detached)", Msg::git("Checkout", &["checkout", &sha]));
    ui.separator();
    menu_item(msgs, ui, "Cherry-pick commit", Msg::git("Cherry-pick", &["cherry-pick", &sha]));
    menu_item(msgs, ui, "Revert commit", Msg::git("Revert", &["revert", "--no-edit", &sha]));
    ui.menu_button("Reset current branch to this commit", |ui| {
        menu_item(msgs, ui, "Soft - keep all changes staged", Msg::git("Reset (soft)", &["reset", "--soft", &sha]));
        menu_item(msgs, ui, "Mixed - keep working copy, reset index", Msg::git("Reset (mixed)", &["reset", "--mixed", &sha]));
        menu_item(
            msgs,
            ui,
            "Hard - discard all changes",
            Msg::confirm(
                "Hard reset",
                format!(
                    "Reset the current branch to {s}? Uncommitted changes to tracked files are stashed first, so Undo brings them back."
                ),
                "Reset",
                Msg::git("Reset (hard)", &["reset", "--hard", &sha]),
            ),
        );
    });
    menu_item(msgs, ui, "Interactive rebase from here…", Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(oid))));
    ui.separator();
    menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(sha.clone()));
    menu_item(msgs, ui, "Copy commit message", Msg::Copy(summary.to_owned()));
}

/// The menu for a row of a multi-selection: actions on all the selected commits.
pub fn multi_commit_menu(ui: &mut egui::Ui, m: &MultiSelection, msgs: &mut Vec<Msg>) {
    ui.label(RichText::new(format!("{} commits selected", m.len())).color(color::weak()));
    ui.separator();
    if cherry_pick_button(ui, m, msgs) {
        ui.close_menu();
    }
    menu_item(msgs, ui, "Copy SHAs", copy_shas(m));
}

/// "Cherry-pick N commits", disabled with the reason on hover when blocked. Returns whether it
/// was clicked.
pub(super) fn cherry_pick_button(ui: &mut egui::Ui, m: &MultiSelection, msgs: &mut Vec<Msg>) -> bool {
    let button = egui::Button::new(format!("Cherry-pick {} commits", m.len()));
    let mut r = ui.add_enabled(m.pick_blocked.is_none(), button);
    if let Some(why) = m.pick_blocked {
        r = r.on_disabled_hover_text(why);
    }
    let clicked = r.clicked();
    if clicked {
        msgs.push(Msg::Selection(SelectionMsg::CherryPick(m.oids.clone())));
    }
    clicked
}

/// Copies the selected commits' full SHAs, one per line, newest first.
pub(super) fn copy_shas(m: &MultiSelection) -> Msg {
    Msg::Copy(m.oids.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(
        "
",
    ))
}

pub fn remote_branch_menu(
    ui: &mut egui::Ui,
    remote: &str,
    name: &str,
    current: &str,
    fs: Option<&ForgeState>,
    filter: &GraphFilter,
    msgs: &mut Vec<Msg>,
) {
    let full = format!("{remote}/{name}");
    menu_item(msgs, ui, format!("Checkout {full}"), Msg::git("Checkout", &["checkout", "--track", &full]));
    if !current.is_empty() {
        menu_item(msgs, ui, format!("Merge {full} into {current}"), Msg::git("Merge", &["merge", &full]));
        menu_item(msgs, ui, format!("Rebase {current} onto {full}"), Msg::git("Rebase", &["rebase", &full]));
    }
    if let Some(fs) = fs.filter(|f| f.remote == remote && f.host.is_some()) {
        ui.separator();
        match fs.pr_for_branch(name) {
            Some(pr) => menu_item(msgs, ui, format!("Open {} {}", fs.pr_noun(), fs.pr_label(pr.id)), Msg::OpenUrl(pr.url.clone())),
            None => menu_item(msgs, ui, format!("Create {}…", fs.pr_noun()), Msg::CreatePrDialog(name.to_owned())),
        }
    }
    menu_item(msgs, ui, "Copy branch name", Msg::Copy(full.clone()));
    ui.separator();
    ref_visibility::menu_items(ui, &format!("refs/remotes/{full}"), &full, filter, false, msgs);
    ui.separator();
    menu_item(
        msgs,
        ui,
        format!("Delete {full} from remote"),
        Msg::confirm(
            "Delete remote branch",
            format!("Delete branch {name} on {remote}? This affects everyone using the remote."),
            "Delete",
            Msg::git("Delete remote branch", &["push", remote, "--delete", name]),
        ),
    );
}
