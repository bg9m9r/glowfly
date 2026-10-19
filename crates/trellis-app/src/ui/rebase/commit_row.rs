//! A commit row of the rebase panel: grip, action, squash connector, SHA, summary, author and
//! date, and its controls (the action combo box, Edit message, Move up and Move down).

use super::rows::{BUTTONS_W, COMBO_W, COMBO_X, GAP, GRIP_X, INDENT, Look, META_W, Place, ROW_H, SHA_X, Shade, fill};
use crate::msg::{Msg, RebaseMsg};
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, icon_button, paint_cut_text, paint_icon};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use trellis_core::rebase::{RebaseAction, TodoItem};

pub(super) fn action_color(a: RebaseAction) -> Color32 {
    match a {
        RebaseAction::Pick => color::weak(),
        RebaseAction::Reword => color::primary(),
        RebaseAction::Squash | RebaseAction::Fixup => color::purple(),
        RebaseAction::Drop => color::red(),
        RebaseAction::Edit => color::amber(),
    }
}

fn action_text(a: RebaseAction) -> RichText {
    RichText::new(a.keyword()).color(action_color(a))
}

/// The row's background, with the floating row's outline, and the grip of a row that moves.
pub(super) fn paint_back(ui: &Ui, rect: Rect, shade: Shade, grip: bool) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), fill(shade));
    if shade == Shade::Floating {
        p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, color::primary()), StrokeKind::Inside);
    }
    if grip {
        paint_icon(p, pos2(rect.left() + GRIP_X, rect.center().y), Icon::Grip, color::faint());
    }
}

/// Everything but the controls: background, grip (`grip`: the row moves), squash connector,
/// SHA, summary, author and date.
pub(super) fn paint_row(ui: &Ui, rect: Rect, item: &TodoItem, look: &Look, shade: Shade, grip: bool) {
    paint_back(ui, rect, shade, grip);
    let p = ui.painter();
    let dropped = item.action == RebaseAction::Drop;
    let mut x = rect.left() + SHA_X;
    if item.action.combines() {
        // An L from the row above into this one: the commit melds into the one before it.
        let (cx, cy) = (x + 6.0, rect.center().y);
        let s = Stroke::new(1.5_f32, color::purple());
        p.line_segment([pos2(cx, rect.top() - GAP), pos2(cx, cy)], s);
        p.line_segment([pos2(cx, cy), pos2(cx + INDENT - 8.0, cy)], s);
        x += INDENT;
    }
    let (text, weak) = if dropped { (color::faint(), color::faint()) } else { (color::text(), color::weak()) };
    let cy = rect.center().y;
    let sha = &item.oid.to_string()[..7];
    let sha_w = paint_cut_text(p, pos2(x, cy), sha, FontId::monospace(12.0), weak, 70.0).rect.width();
    x += sha_w + 10.0;

    let date = look.settings.format_time(item.time, look.now, look.offset);
    let author = if item.author.is_empty() { "Unknown" } else { item.author.as_str() };
    let meta = format!("{author}, {date}");
    let meta_w = ui.fonts(|f| f.layout_no_wrap(meta.clone(), FontId::proportional(12.0), weak).size().x).min(META_W - 10.0);
    let meta_x = rect.right() - BUTTONS_W - 10.0 - meta_w;
    paint_cut_text(p, pos2(meta_x, cy), &meta, FontId::proportional(12.0), weak, META_W - 10.0);

    let summary = shown_summary(item);
    let w = paint_cut_text(p, pos2(x, cy), summary, FontId::proportional(13.5), text, meta_x - x - 16.0).rect.width();
    if dropped && w > 0.0 {
        p.line_segment([pos2(x, cy + 0.5), pos2(x + w, cy + 0.5)], Stroke::new(1.0_f32, text));
    }
}

/// The summary: the first line of a group head's typed combined message, or of a reword's
/// typed message, else the commit's own.
fn shown_summary(item: &TodoItem) -> &str {
    match (&item.combined_message, &item.new_message) {
        (Some(m), _) => m.lines().next().unwrap_or_default(),
        (None, Some(m)) if item.action == RebaseAction::Reword => m.lines().next().unwrap_or_default(),
        _ => &item.summary,
    }
}

/// A label painted in a chip where the action combo box sits (the action while rows move, or
/// a step's kind).
pub(super) fn paint_chip(ui: &Ui, row: Rect, text: &str, c: Color32) {
    let rect = Rect::from_min_size(pos2(row.left() + COMBO_X, row.center().y - 11.0), vec2(COMBO_W, 22.0));
    ui.painter().rect_filled(rect, CornerRadius::same(3), color::toolbar());
    let pos = pos2(rect.left() + 8.0, rect.center().y);
    ui.painter().text(pos, Align2::LEFT_CENTER, text, FontId::proportional(13.0), c);
}

/// The action as a painted label in place of its combo box (while rows move).
pub(super) fn paint_action(ui: &Ui, row: Rect, a: RebaseAction) {
    paint_chip(ui, row, a.keyword(), action_color(a));
}

/// The row's controls: what it is, which actions its combo box offers, and whether it is fixed
/// in place (no move buttons).
pub(super) struct RowKind<'a> {
    pub head: bool,
    pub actions: &'a [RebaseAction],
    pub fixed: bool,
}

/// The action combo box, Edit message (the combined one on a group head) and the move buttons
/// of the row at `at`.
pub(super) fn controls(ui: &mut Ui, rect: Rect, at: Place, item: &TodoItem, kind: RowKind, msgs: &mut Vec<Msg>) {
    let RowKind { head, actions, fixed } = kind;
    let Place { i, enabled, .. } = at;
    let combo_rect = Rect::from_min_size(pos2(rect.left() + COMBO_X, rect.top()), vec2(COMBO_W, ROW_H));
    let mut child = ui.new_child(UiBuilder::new().max_rect(combo_rect).layout(Layout::left_to_right(Align::Center)));
    if !enabled {
        child.disable();
    }
    let mut action = item.action;
    egui::ComboBox::from_id_salt(("rebase-action", item.oid)).width(COMBO_W - 8.0).selected_text(action_text(action)).show_ui(
        &mut child,
        |ui| {
            for &a in actions {
                ui.selectable_value(&mut action, a, action_text(a));
            }
        },
    );
    if action != item.action {
        msgs.push(Msg::Rebase(RebaseMsg::RebaseSetAction { index: i, action }));
    }
    let tip = if head { "Edit combined message" } else { "Edit message" };
    let edit = Some((Icon::Pencil, tip, RebaseMsg::RebaseEditMessage(i)));
    move_buttons(ui, rect, at, Buttons { edit, remove: None, moves: !fixed }, msgs);
}

/// The buttons at the right of a row: an optional edit and remove button, and whether Move up
/// and Move down are shown.
pub(super) struct Buttons<'a> {
    pub edit: Option<(Icon, &'a str, RebaseMsg)>,
    pub remove: Option<RebaseMsg>,
    pub moves: bool,
}

/// The buttons at the right of the row at `at`: an optional edit and remove button, then Move
/// up and Move down.
pub(super) fn move_buttons(ui: &mut Ui, rect: Rect, at: Place, b: Buttons, msgs: &mut Vec<Msg>) {
    let Place { i, n, enabled } = at;
    let Buttons { edit, remove, moves } = b;
    let buttons = Rect::from_min_max(pos2(rect.right() - BUTTONS_W, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
    let mut child = ui.new_child(UiBuilder::new().max_rect(buttons).layout(Layout::right_to_left(Align::Center)));
    child.spacing_mut().item_spacing.x = 3.0;
    if !enabled {
        child.disable();
    }
    // Right to left: Move down, Move up, then Remove and Edit.
    if moves {
        let down = child.add_enabled_ui(i + 1 < n, |ui| icon_button(ui, Icon::NextChange, "Move down")).inner;
        if down.clicked() {
            msgs.push(Msg::Rebase(RebaseMsg::RebaseMove { from: i, to: i + 1 }));
        }
        let up = child.add_enabled_ui(i > 0, |ui| icon_button(ui, Icon::PrevChange, "Move up")).inner;
        if up.clicked() {
            msgs.push(Msg::Rebase(RebaseMsg::RebaseMove { from: i, to: i - 1 }));
        }
    }
    if let Some(msg) = remove
        && icon_button(&mut child, Icon::Close, "Remove step").clicked()
    {
        msgs.push(Msg::Rebase(msg));
    }
    if let Some((icon, tip, msg)) = edit
        && icon_button(&mut child, icon, tip).clicked()
    {
        msgs.push(Msg::Rebase(msg));
    }
}
