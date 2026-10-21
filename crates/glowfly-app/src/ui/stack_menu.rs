//! The branch menu's stack parts: the header's note naming the branch below ("Stacked on
//! search · needs restack") and the stack items, shown only for a branch in a stack: restack,
//! Push stack and, with a code host, creating the stack's pull requests. Disabled items say why
//! on hover, in the words `update` refuses with (`state::stack::blocker`,
//! `state::stack_push::push_blocker`, `state::stack_prs::prs_blocker`).

use super::common::{menu_header, menu_header_note, menu_item_if};
use crate::msg::{Msg, RebaseMsg, StackMsg};
use crate::state::ForgeState;
use crate::state::stack::blocker;
use crate::state::stack_prs::prs_blocker;
use crate::state::stack_push::push_blocker;
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::stack::RestackScope;

/// The menu's header: the branch's name, with the branch it is stacked on under it.
pub fn header(ui: &mut egui::Ui, t: Lang, name: &str, snap: &Snapshot) {
    match snap.stacks.below(name) {
        None => menu_header(ui, name),
        Some(l) => {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &l.on)];
            let note = if l.behind { t.trf("stack.menu.on_behind", args) } else { t.trf("stack.menu.on", args) };
            menu_header_note(ui, name, &note);
        }
    }
}

/// After a separator: "Restack branches above" when branches are stacked on `b`, "Restack onto
/// <lower>" when `b`'s own link is behind, Push stack, and with a code host the stack's pull
/// requests; nothing for a branch outside a stack.
pub fn items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let has_above = !snap.stacks.above(name).is_empty();
    let below = snap.stacks.below(name);
    if !has_above && below.is_none() {
        return;
    }
    ui.separator();
    let mut item = |ui: &mut egui::Ui, label: String, scope: RestackScope| {
        let why = blocker(t, snap, &scope);
        menu_item_if(msgs, ui, label, why.as_deref(), Msg::Rebase(RebaseMsg::Restack { scope, autostash: false }));
    };
    if has_above {
        item(ui, t.tr("stack.menu.restack_above").to_owned(), RestackScope::Above(name.to_owned()));
    }
    if let Some(l) = below.filter(|l| l.behind) {
        item(ui, t.trf("stack.menu.restack_onto", &[("branch", &l.on)]), RestackScope::Onto(name.to_owned()));
    }
    let branch = name.to_owned();
    let why = push_blocker(t, snap, name);
    menu_item_if(msgs, ui, t.tr("stack.menu.push"), why.as_deref(), Msg::Stack(StackMsg::Push { branch: branch.clone() }));
    if let Some(fs) = fs.filter(|f| f.host.is_some()) {
        let label = if fs.pr_noun() == "merge request" { t.tr("stack.menu.create_mrs") } else { t.tr("stack.menu.create_prs") };
        let why = prs_blocker(t, snap, fs, name);
        menu_item_if(msgs, ui, label, why.as_deref(), Msg::Stack(StackMsg::CreatePrs { branch }));
    }
}
