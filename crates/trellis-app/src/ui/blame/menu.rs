//! A blame line's menu: blame before its commit or skip it, the file history and the graph, and
//! copying the line or the commit's SHA.

use crate::msg::{BlameMsg, Msg};
use crate::ui::common::menu_item_if;
use crate::ui::history::NOT_ON_GRAPH;
use eframe::egui::Ui;
use trellis_core::repo::BlameCommit;

pub(in crate::ui) const BLAME_BEFORE: &str = "Blame before this commit";
pub(in crate::ui) const SKIP: &str = "Skip this commit";
pub(in crate::ui) const SHOW_IN_HISTORY: &str = "Show in file history";
pub(in crate::ui) const SELECT_ON_GRAPH: &str = "Select on the graph";
pub(in crate::ui) const COPY_LINE: &str = "Copy line";
pub(in crate::ui) const COPY_SHA: &str = "Copy commit SHA";

/// Why Blame before this commit is off for a commit with nothing before it.
pub(in crate::ui) const NOTHING_BEFORE: &str = "This commit added these lines with the file, or older history is not in this clone";
const NOT_COMMITTED: &str = "These lines are not committed yet";

/// The line a menu acts on.
pub(in crate::ui) struct LineTarget<'a> {
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

pub(in crate::ui) fn line_menu(ui: &mut Ui, msgs: &mut Vec<Msg>, t: &LineTarget) {
    let oid = t.commit.oid;
    let zero = oid.is_zero();
    let before = if zero {
        Some(NOT_COMMITTED)
    } else if !t.before {
        Some(NOTHING_BEFORE)
    } else {
        None
    };
    menu_item_if(msgs, ui, BLAME_BEFORE, before, Msg::Blame(BlameMsg::BlameParent { line: t.line }));
    let skip = if zero {
        Some(NOT_COMMITTED)
    } else if t.skipped {
        Some("Already skipped")
    } else {
        None
    };
    menu_item_if(msgs, ui, SKIP, skip, Msg::Blame(BlameMsg::SkipCommit(oid)));
    ui.separator();
    menu_item_if(msgs, ui, SHOW_IN_HISTORY, None, Msg::Blame(BlameMsg::OpenHistory { select: Some(oid) }));
    let graph = (!t.on_graph).then_some(NOT_ON_GRAPH);
    menu_item_if(msgs, ui, SELECT_ON_GRAPH, graph, Msg::Blame(BlameMsg::SelectCommit(oid)));
    ui.separator();
    menu_item_if(msgs, ui, COPY_LINE, None, Msg::Copy(t.text.to_owned()));
    menu_item_if(msgs, ui, COPY_SHA, None, Msg::Copy(oid.to_string()));
}
