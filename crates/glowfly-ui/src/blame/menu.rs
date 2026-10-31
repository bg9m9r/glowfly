//! A blame line's menu: blame before its commit or skip it, the file history and the graph, and
//! copying the line or the commit's SHA.

use crate::msg::{BlameMsg, Msg};
use crate::ui::common::menu_item_if;
use crate::ui::widgets::menu::separator;
use eframe::egui::Ui;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::BlameCommit;

/// The line a menu acts on.
pub(crate) struct LineTarget<'a> {
    /// 0-based.
    pub line: usize,
    pub text: &'a str,
    pub commit: &'a BlameCommit,
    /// Its origin has a parent to blame.
    pub before: bool,
    /// Its commit is skipped already.
    pub skipped: bool,
    /// Its commit is on the graph.
    pub on_graph: bool,
}

pub(crate) fn line_menu(ui: &mut Ui, t: Lang, msgs: &mut Vec<Msg>, line: &LineTarget) {
    let oid = line.commit.oid;
    let zero = oid.is_zero();
    let not_committed = t.tr("blame.menu.not_committed");
    let before = if zero {
        Some(not_committed)
    } else if !line.before {
        // Nothing before it: the commit added the file, or older history isn't in this clone.
        Some(t.tr("blame.menu.nothing_before"))
    } else {
        None
    };
    menu_item_if(msgs, ui, t.tr("blame.menu.blame_before"), before, Msg::Blame(BlameMsg::BlameParent { line: line.line }));
    let skip = if zero {
        Some(not_committed)
    } else if line.skipped {
        Some(t.tr("blame.menu.already_skipped"))
    } else {
        None
    };
    menu_item_if(msgs, ui, t.tr("blame.menu.skip"), skip, Msg::Blame(BlameMsg::SkipCommit(oid)));
    separator(ui);
    menu_item_if(msgs, ui, t.tr("blame.menu.show_in_history"), None, Msg::Blame(BlameMsg::OpenHistory { select: Some(oid) }));
    let graph = (!line.on_graph).then(|| t.tr("blame.menu.not_on_graph"));
    menu_item_if(msgs, ui, t.tr("blame.menu.select_on_graph"), graph, Msg::Blame(BlameMsg::SelectCommit(oid)));
    separator(ui);
    menu_item_if(msgs, ui, t.tr("blame.menu.copy_line"), None, Msg::Copy(line.text.to_owned()));
    menu_item_if(msgs, ui, t.tr("blame.menu.copy_sha"), None, Msg::Copy(oid.to_string()));
}
