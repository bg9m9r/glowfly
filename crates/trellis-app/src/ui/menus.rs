//! Right-click menus for commits and branches.

use super::common::{menu_item, short};
use crate::msg::Msg;
use crate::state::ForgeState;
use eframe::egui;
use git2::Oid;
use trellis_core::repo::BranchInfo;

pub fn commit_menu(ui: &mut egui::Ui, oid: Oid, summary: &str, msgs: &mut Vec<Msg>) {
    let sha = oid.to_string();
    let s = short(oid);
    menu_item(msgs, ui, "Create branch here…", Msg::NewBranch { start: Some(sha.clone()), name: String::new() });
    menu_item(msgs, ui, "Create tag here…", Msg::NewTag(sha.clone()));
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
                format!("Reset the current branch to {s} and discard all uncommitted changes?"),
                "Reset",
                Msg::git("Reset (hard)", &["reset", "--hard", &sha]),
            ),
        );
    });
    ui.separator();
    menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(sha.clone()));
    menu_item(msgs, ui, "Copy commit message", Msg::Copy(summary.to_owned()));
}

pub fn local_branch_menu(ui: &mut egui::Ui, b: &BranchInfo, current: &str, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    if !b.is_head {
        menu_item(msgs, ui, format!("Checkout {name}"), Msg::git("Checkout", &["checkout", name]));
        if !current.is_empty() {
            menu_item(msgs, ui, format!("Merge {name} into {current}"), Msg::git("Merge", &["merge", name]));
            menu_item(msgs, ui, format!("Rebase {current} onto {name}"), Msg::git("Rebase", &["rebase", name]));
        }
    }
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
        ui.separator();
    }
    if let Some(o) = b.oid {
        menu_item(msgs, ui, "Create branch here…", Msg::NewBranch { start: Some(o.to_string()), name: String::new() });
    }
    menu_item(msgs, ui, "Copy branch name", Msg::Copy(name.to_owned()));
    if !b.is_head {
        ui.separator();
        menu_item(
            msgs,
            ui,
            format!("Delete {name}"),
            Msg::confirm(
                "Delete branch",
                format!("Delete local branch {name}?"),
                "Delete",
                Msg::git("Delete branch", &["branch", "-d", name]),
            ),
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
    }
}

pub fn remote_branch_menu(ui: &mut egui::Ui, remote: &str, name: &str, current: &str, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
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
