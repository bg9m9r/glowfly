//! The diff's right-click menu over a line's code: Copy (its key beside it; only with text
//! selected), Copy with diff signs, Copy lines (the whole lines under the selection, or the line
//! clicked), Copy hunk as patch (`DiffMsg::CopyHunkPatch`), then, behind a separator, the hunk
//! header's line actions (Stage, Unstage, Discard, Revert or Comment on N lines): on the selected
//! lines, or with none on the changed line clicked. On an unchanged line they don't show. A
//! right-click inside the selected text keeps it and anywhere else drops it; Shift+F10 opens the
//! menu below the topmost visible selected line while the diff has the keyboard.

use super::DiffView;
use super::text_sel::{self, Hit, TextMut};
use crate::keymap::{Chord, KeyName};
use crate::msg::{DiffMsg, Msg, StageMsg};
use crate::state::diff_text::{self, LineRef, TextSel, TextSide};
use crate::state::lines::LineClick;
use crate::ui::widgets::menu;
use crate::ui::widgets::reason::ItemState;
use eframe::egui::{Pos2, Response, Ui};
use glowfly_core::repo::{DiffTarget, HunkOp};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::pull_request::line_comment::LineCommentMsg;

/// The line the menu is for: where the right-click landed, or the line Shift+F10 opened it at.
type At = Option<(TextSide, LineRef)>;

/// After the rows: the right-click and Shift+F10 on the code (`bg`), against the lines drawn
/// (`hits`), and the menu while it is open. A right-click outside the selected text drops it.
pub(super) fn handle(ui: &Ui, bg: &Response, hits: &[Hit], v: &DiffView, t: &mut TextMut, msgs: &mut Vec<Msg>) {
    let ctx = ui.ctx();
    let at_id = text_sel::body_id().with("menu-at");
    let open = menu::context_menu_open(bg);
    let mut on_row = false;
    if bg.secondary_clicked() {
        let hit = bg.interact_pointer_pos().and_then(|p| hits.iter().find(|h| h.cell.contains(p) && p.x >= h.text_x).map(|h| (h, p)));
        on_row = hit.is_some();
        if !hit.is_some_and(|(h, p)| t.sel.as_ref().is_some_and(|s| inside(v, s, h, p))) {
            t.sel = None;
        }
        ctx.data_mut(|d| d.insert_temp::<At>(at_id, hit.map(|(h, _)| (h.side, h.line))));
    }
    // From the keyboard: below the topmost visible line selected, as text or in the gutter.
    let keyed = (!open && text_sel::focused(ctx)).then(|| hits.iter().find(|h| selected(v, h))).flatten();
    if !open && !bg.secondary_clicked() {
        ctx.data_mut(|d| d.insert_temp::<At>(at_id, keyed.map(|h| (h.side, h.line))));
    }
    let at = ctx.data(|d| d.get_temp::<At>(at_id)).flatten();
    let sel = t.sel.as_ref();
    menu::context_menu_at(bg, on_row, keyed.map(|h| h.cell), |ui| match at {
        Some(at) => items(ui, v, sel, at, msgs),
        None => ui.close_menu(),
    });
}

/// Whether the pointer at `p` over `h` is inside the selected text `s`.
fn inside(v: &DiffView, s: &TextSel, h: &Hit, p: Pos2) -> bool {
    let Some((side, span)) = text_sel::span(v, s) else { return false };
    let Some(row) = v.text.index.row(h.line) else { return false };
    let here = (row, text_sel::col_at(h, p, v.style.tab));
    side == h.side && !span.is_empty() && (span.start_at, span.start.col) <= here && here <= (span.end_at, span.end.col)
}

/// Whether line `h` is selected: some of its text, or the line itself in the gutter.
fn selected(v: &DiffView, h: &Hit) -> bool {
    let picked = matches!(h.line, LineRef::Hunk { hunk, line } if v.is_selected(hunk, line));
    picked || v.text.highlight(h.side, h.line, h.raw).is_some()
}

/// The menu's items for line `at`, with the text selection `sel`.
fn items(ui: &mut Ui, v: &DiffView, sel: Option<&TextSel>, at: (TextSide, LineRef), msgs: &mut Vec<Msg>) {
    let t = v.t;
    let side = sel.map_or(at.0, |s| s.side);
    let any = sel.is_some_and(|s| text_sel::span(v, s).is_some_and(|(_, span)| !span.is_empty()));
    let no_text = (!any).then(|| t.tr("diff.row_menu.no_text"));
    let copy_key = Chord::cmd(KeyName::Letter('C')).text();
    let mut copy = |ui: &mut Ui, row: menu::Item, text: &dyn Fn() -> String| {
        if row.show(ui).clicked() {
            msgs.push(Msg::Copy(text()));
            ui.close_menu();
        }
    };
    let copied = |signs| sel.map(|s| text_sel::copy(v, s, signs)).unwrap_or_default();
    copy(ui, menu::item(t.tr("diff.row_menu.copy")).shortcut(copy_key).blocked(no_text), &|| copied(false));
    copy(ui, menu::item(t.tr("diff.row_menu.copy_signs")).blocked(no_text), &|| copied(true));
    copy(ui, menu::item(t.tr("diff.row_menu.copy_lines")), &|| diff_text::copy_lines(&text_sel::shown(v, side), sel, Some(at.1)));
    copy_hunk(ui, v, at.1, msgs);
    line_actions(ui, v, at.1, msgs);
}

/// Copy hunk as patch for the hunk line `r` is in, read again from the repository
/// (`DiffMsg::CopyHunkPatch`) so the patch has the file's own bytes. Disabled off a hunk, while
/// a reload is on its way, and for a hunk whose text isn't UTF-8 (shown with U+FFFD), which
/// can't go to the clipboard as text.
fn copy_hunk(ui: &mut Ui, v: &DiffView, r: LineRef, msgs: &mut Vec<Msg>) {
    let t = v.t;
    let hunk = hunk_of(v, r);
    let not_text = |h: usize| v.diff.hunks.get(h).is_some_and(|h| h.lines.iter().any(|l| l.text.contains(char::REPLACEMENT_CHARACTER)));
    let why = match (hunk, v.hunk_gen) {
        (None, _) => Some(t.tr("diff.row_menu.no_hunk")),
        (Some(_), None) => Some(t.tr("diff.hunk.updating")),
        (Some(h), Some(_)) if not_text(h) => Some(t.tr("diff.row_menu.not_text")),
        _ => None,
    };
    if menu::item(t.tr("diff.row_menu.copy_hunk")).blocked(why).show(ui).clicked()
        && let (Some(index), Some(generation)) = (hunk, v.hunk_gen)
    {
        msgs.push(Msg::Diff(DiffMsg::CopyHunkPatch { index, generation }));
        ui.close_menu();
    }
}

/// The normal hunk line `r` is in: its own, or in the whole-file view the hunk holding the same
/// line; `None` for an unchanged line outside every hunk (a gap's, or far from any change).
fn hunk_of(v: &DiffView, r: LineRef) -> Option<usize> {
    let line = match r {
        LineRef::Hunk { hunk, .. } => return Some(hunk),
        LineRef::Gap { .. } => return None,
        LineRef::Whole { line } => v.lines().hunks.first()?.lines.get(line)?,
    };
    let same = |l: &glowfly_core::repo::DiffLine| l.origin == line.origin && l.old == line.old && l.new == line.new;
    v.diff.hunks.iter().position(|h| h.lines.iter().any(same))
}

/// What the line actions act on: the selected lines of the diff, or with none the changed line
/// `r` clicked (selected first). `None` on an unchanged line or where lines can't be selected.
struct Lines {
    /// How many lines; Comment on N lines counts its own (`comment`).
    count: usize,
    comment: usize,
    /// Selects the clicked line first, when nothing is selected.
    pick: Option<Msg>,
}

fn lines_for(v: &DiffView, r: LineRef) -> Option<Lines> {
    let (sel, LineRef::Hunk { hunk, line }) = (v.sel?, r) else { return None };
    let dl = v.diff.hunks.get(hunk)?.lines.get(line).filter(|l| matches!(l.origin, '+' | '-'))?;
    let commentable = v.comments.is_some_and(|c| c.blocked.is_none());
    let picked = sel.target.as_ref() == Some(v.target) && !sel.lines.is_empty();
    Some(if picked {
        Lines { count: sel.lines.len(), comment: super::hunk_header::comment_lines(v, sel.hunk), pick: None }
    } else {
        let comment = usize::from(commentable && glowfly_core::forge::line_key(dl).is_some());
        Lines { count: 1, comment, pick: Some(Msg::Diff(DiffMsg::SelectDiffLine { hunk, line, how: LineClick::Only })) }
    })
}

/// The hunk header's line actions for the diff's target, behind a separator: none on an
/// unchanged line, and disabled while a reload is on its way.
fn line_actions(ui: &mut Ui, v: &DiffView, r: LineRef, msgs: &mut Vec<Msg>) {
    let Some(l) = lines_for(v, r) else { return };
    let t = v.t;
    let path = v.target.path();
    let mut list: Vec<(String, ItemState, Msg)> = Vec::new();
    let ready = |danger: bool| match (v.hunk_gen, danger) {
        (None, _) => ItemState::Blocked(t.tr("diff.hunk.updating")),
        (Some(_), true) => ItemState::Danger,
        (Some(_), false) => ItemState::Ready,
    };
    let n = l.count;
    match v.target {
        DiffTarget::Unstaged(_) => {
            list.push((t.trn("diff.hunk.stage_lines", n, &[]), ready(false), Msg::Stage(StageMsg::ApplyLines(HunkOp::Stage))));
            let body = t.trn("diff.discard_lines.body", n, &[("path", &path)]);
            let then = Msg::Stage(StageMsg::ApplyLines(HunkOp::Discard));
            let ask = Msg::confirm_danger(t.tr("diff.discard_lines.title"), body, t.tr("common.discard.button"), then);
            list.push((t.trn("diff.hunk.discard_lines", n, &[]), ready(true), ask));
        }
        DiffTarget::Staged(_) => {
            list.push((t.trn("diff.hunk.unstage_lines", n, &[]), ready(false), Msg::Stage(StageMsg::ApplyLines(HunkOp::Unstage))));
        }
        DiffTarget::Commit(..) | DiffTarget::Between(..) | DiffTarget::ToWorkdir(..) => {
            list.push((t.trn("diff.hunk.revert_lines", n, &[]), ready(false), Msg::Stage(StageMsg::RevertLines)));
        }
        DiffTarget::FromWorkdir(..) => {}
    }
    if l.comment > 0 {
        let start = Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::OnSelection));
        list.push((t.trn("diff.hunk.comment_lines", l.comment, &[]), ready(false), start));
    }
    if list.is_empty() {
        return;
    }
    menu::separator(ui);
    let mut pick = l.pick;
    for (label, state, m) in list {
        if menu::item(label).state(state).show(ui).clicked() {
            msgs.extend(pick.take());
            msgs.push(m);
            ui.close_menu();
        }
    }
}
