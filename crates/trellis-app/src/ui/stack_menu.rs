//! The branch menu's stack parts: the header's note naming the branch below ("Stacked on
//! search · needs restack") and the restack items, shown only for a branch in a stack. Disabled
//! items say why on hover, in the words `update` refuses with (`state::stack::blocker`).

use super::common::{menu_header, menu_header_note, menu_item_if};
use crate::msg::{Msg, RebaseMsg};
use crate::state::stack::blocker;
use eframe::egui;
use trellis_core::i18n::Lang;
use trellis_core::repo::{BranchInfo, Snapshot};
use trellis_core::stack::RestackScope;

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

/// "Restack branches above" when branches are stacked on `b`, and "Restack onto <lower>" when
/// `b`'s own link is behind, after a separator; nothing for a branch outside a stack.
pub fn items(ui: &mut egui::Ui, t: Lang, b: &BranchInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let name = b.name.as_str();
    let has_above = !snap.stacks.above(name).is_empty();
    let behind = snap.stacks.below(name).filter(|l| l.behind);
    if !has_above && behind.is_none() {
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
    if let Some(l) = behind {
        item(ui, t.trf("stack.menu.restack_onto", &[("branch", &l.on)]), RestackScope::Onto(name.to_owned()));
    }
}
