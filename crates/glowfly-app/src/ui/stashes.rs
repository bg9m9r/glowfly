//! The STASHES section of the left panel: one row per stash (a click selects its commit) with a
//! menu to apply, pop, rename or delete it; an auto-stash shows a weak "auto" on the right. The header's + opens the Stash dialog, and its menu
//! stashes every change or only the staged ones.

use super::common::menu_item;
use super::theme::color;
use super::widgets;
use super::widgets::section_header::{self, Header, HeaderAdd, action_item};
use crate::msg::{Msg, RepoMsg, StashMsg};
use crate::state::Selection;
use crate::state::palette::StashAction;
use crate::state::section_actions::SectionAction;
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::stash_ops;

/// Draws the section; `all` and `staged` are the header's Stash changes and Stash staged
/// changes.
pub(super) fn section(
    ui: &mut egui::Ui,
    t: Lang,
    snap: &Snapshot,
    open: &mut bool,
    (sel, shown): (&Selection, &dyn Fn(&str) -> bool),
    (all, staged): (&SectionAction, &SectionAction),
    msgs: &mut Vec<Msg>,
) {
    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
        action_item(ui, msgs, all);
        action_item(ui, msgs, staged);
    };
    let title = t.tr("sidebar.stashes.header");
    let h = Header { key: "stashes", title, count: Some(snap.stashes.len()), add: Some(HeaderAdd::send(all)), menu: Some(&menu), t };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    for s in snap.stashes.iter().filter(|s| shown(&s.message)) {
        if widgets::list_row_hidden(ui) {
            continue;
        }
        // Made before an operation (by Trellis, or git's own kept autostash): a weak "auto".
        let auto = stash_ops::is_auto_stash(&s.message);
        let mark = auto.then(|| (t.tr("sidebar.stashes.auto"), color::weak()));
        let mut r = widgets::list_row(ui, widgets::ROW_INDENT, &s.message, mark, *sel == Selection::Commit(s.oid), false);
        if auto {
            r = r.on_hover_text(t.tr("sidebar.stashes.auto.tip"));
        }
        let stash = |action| crate::state::stash_confirm::stash_msg(snap, action, s);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::SelectCommit(s.oid)));
        }
        r.context_menu(|ui| {
            menu_item(msgs, ui, "Apply", stash(StashAction::Apply));
            menu_item(msgs, ui, "Pop", stash(StashAction::Pop));
            menu_item(msgs, ui, "Rename…", Msg::Stash(StashMsg::RenameDialog { oid: s.oid }));
            menu_item(msgs, ui, "Delete", stash(StashAction::Drop));
        });
    }
}
