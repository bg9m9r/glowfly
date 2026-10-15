//! The rows of the rebase panel: a drag grip, the action, the commit, its author and date, and
//! move buttons; the inline message editor under the row being reworded; and dragging rows to
//! reorder them.

use super::super::spell_edit::spell_text_edit;
use super::super::theme::color;
use super::super::widgets::{Icon, icon_button, paint_icon};
use super::UiCache;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::rebase::RebasePanel;
use crate::state::tabs::index_after_move;
use crate::state::{AppState, Selection};
use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Layout, PointerButton, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use trellis_core::rebase::{RebaseAction, TodoItem};
use trellis_core::settings::Settings;

const ROW_H: f32 = 38.0;
const GAP: f32 = 4.0;
const MARGIN: f32 = 12.0;
const GRIP_X: f32 = 16.0;
const COMBO_X: f32 = 32.0;
const COMBO_W: f32 = 96.0;
const SHA_X: f32 = 140.0;
const INDENT: f32 = 22.0;
const BUTTONS_W: f32 = 60.0;
const META_W: f32 = 230.0;

/// A row being dragged: its index and where on it the pointer grabbed it (from its top edge).
/// Kept in egui's temporary memory, since the press position is gone by the release frame.
#[derive(Clone, Copy)]
struct RowDrag {
    from: usize,
    grab: f32,
}

/// A drag in progress: the dragged row, the slot it would drop into, and where it is drawn.
#[derive(Clone, Copy)]
struct Floating {
    from: usize,
    to: usize,
    rect: Rect,
}

/// One row's place in the list and its interactions this frame.
struct Slot {
    rect: Rect,
    r: Response,
}

/// What a row is drawn with besides its item.
struct Look<'a> {
    settings: &'a Settings,
    now: i64,
    offset: i32,
}

pub(super) fn list(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let look = Look { settings: &st.settings, now: st.view.now, offset: st.view.utc_offset };
    let spell = st.settings.spellcheck;
    let selected = match st.selection {
        Selection::Commit(o) => Some(o),
        _ => None,
    };
    let Some(panel) = st.rebase.as_mut() else { return };
    let enabled = panel.editable();
    let can_drag = enabled && panel.editing.is_none() && panel.items.len() > 1;
    egui::ScrollArea::vertical().id_salt("rebase-list").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.y = GAP;
        let n = panel.items.len();
        let mut slots = Vec::with_capacity(n);
        let list_top = ui.cursor().top();
        for i in 0..n {
            let width = ui.available_width() - 2.0 * MARGIN;
            let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
            let rect = Rect::from_min_size(pos2(outer.left() + MARGIN, outer.top()), vec2(width, ROW_H));
            let item = &panel.items[i];
            let sense = if can_drag { Sense::click_and_drag() } else { Sense::click() };
            let r = ui.interact(rect, ui.id().with(("rebase-row", item.oid)), sense);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, enabled, &item.summary));
            slots.push(Slot { rect, r });
            if panel.editing == Some(i) {
                editor(ui, panel, cache, spell, msgs);
            }
        }
        let floating = if can_drag { drag(ui, &slots, list_top, ROW_H + GAP, msgs) } else { None };
        for (i, Slot { rect, r }) in slots.iter().enumerate() {
            let item = &panel.items[i];
            match floating {
                Some(f) if f.from == i => {}
                // While dragging, the others slide to where they would be after the drop.
                Some(f) => {
                    let shift = (index_after_move(i, f.from, f.to) as f32 - i as f32) * (ROW_H + GAP);
                    paint_row(ui, rect.translate(vec2(0.0, shift)), item, &look, Shade::Plain);
                    paint_action(ui, rect.translate(vec2(0.0, shift)), item.action);
                }
                None => {
                    let shade = if selected == Some(item.oid) {
                        Shade::Selected
                    } else if r.hovered() {
                        Shade::Hovered
                    } else {
                        Shade::Plain
                    };
                    paint_row(ui, *rect, item, &look, shade);
                    controls(ui, *rect, i, n, item, enabled, msgs);
                    if can_drag && grip_rect(*rect).contains(ui.input(|inp| inp.pointer.hover_pos()).unwrap_or_default()) {
                        ui.ctx().set_cursor_icon(CursorIcon::Grab);
                    }
                    if r.clicked() {
                        msgs.push(Msg::Repo(RepoMsg::SelectCommit(item.oid)));
                    }
                    let on_summary = r.interact_pointer_pos().is_some_and(|p| p.x >= rect.left() + SHA_X);
                    if enabled && r.double_clicked() && on_summary {
                        msgs.push(Msg::RebaseEditMessage(i));
                    }
                }
            }
        }
        // The dragged row, drawn last so it floats above the others.
        if let Some(f) = floating {
            let item = &panel.items[f.from];
            paint_row(ui, f.rect, item, &look, Shade::Floating);
            paint_action(ui, f.rect, item.action);
        }
        ui.add_space(12.0);
    });
}

/// Tracks a row drag: returns where the dragged row floats while one is in progress, and
/// pushes `RebaseMove` when it is dropped in another slot.
fn drag(ui: &Ui, slots: &[Slot], list_top: f32, pitch: f32, msgs: &mut Vec<Msg>) -> Option<Floating> {
    let drag_id = Id::new("rebase-drag");
    let n = slots.len();
    if let Some((i, s)) = slots.iter().enumerate().find(|(_, s)| s.r.drag_started_by(PointerButton::Primary))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(drag_id, RowDrag { from: i, grab: origin.y - s.rect.top() }));
    }
    let state = ui.data(|d| d.get_temp::<RowDrag>(drag_id))?;
    let slot = slots.get(state.from).filter(|s| s.r.dragged() || s.r.drag_stopped());
    let (Some(slot), Some(p)) = (slot, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this row (e.g. the list changed under it).
        ui.data_mut(|d| d.remove::<RowDrag>(drag_id));
        return None;
    };
    let top = (p.y - state.grab).clamp(list_top, list_top + (n - 1) as f32 * pitch);
    let to = drop_slot(top, list_top, pitch, n);
    if slot.r.drag_stopped() {
        ui.data_mut(|d| d.remove::<RowDrag>(drag_id));
        if to != state.from {
            msgs.push(Msg::RebaseMove { from: state.from, to });
        }
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Floating { from: state.from, to, rect: Rect::from_min_size(pos2(slot.rect.left(), top), slot.rect.size()) })
}

/// The slot nearest to a row whose top edge is at `top`.
fn drop_slot(top: f32, list_top: f32, pitch: f32, n: usize) -> usize {
    (((top - list_top) / pitch).round().max(0.0) as usize).min(n - 1)
}

fn grip_rect(row: Rect) -> Rect {
    Rect::from_center_size(pos2(row.left() + GRIP_X, row.center().y), vec2(20.0, ROW_H))
}

#[derive(Clone, Copy, PartialEq)]
enum Shade {
    Plain,
    Hovered,
    Selected,
    Floating,
}

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

/// Everything but the controls: background, grip, squash connector, SHA, summary, author and
/// date.
fn paint_row(ui: &Ui, rect: Rect, item: &TodoItem, look: &Look, shade: Shade) {
    let p = ui.painter();
    let fill = match shade {
        Shade::Plain | Shade::Floating => color::panel(),
        Shade::Hovered => color::hover(),
        Shade::Selected => color::selected(),
    };
    p.rect_filled(rect, CornerRadius::same(4), fill);
    if shade == Shade::Floating {
        p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, color::primary()), StrokeKind::Inside);
    }
    paint_icon(p, pos2(rect.left() + GRIP_X, rect.center().y), Icon::Grip, color::faint());

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
    let sha_w = line(ui, pos2(x, cy), sha, FontId::monospace(12.0), weak, 70.0);
    x += sha_w + 10.0;

    let date = look.settings.format_time(item.time, look.now, look.offset);
    let author = if item.author.is_empty() { "Unknown" } else { item.author.as_str() };
    let meta = format!("{author}, {date}");
    let meta_w = ui.fonts(|f| f.layout_no_wrap(meta.clone(), FontId::proportional(12.0), weak).size().x).min(META_W - 10.0);
    let meta_x = rect.right() - BUTTONS_W - 10.0 - meta_w;
    line(ui, pos2(meta_x, cy), &meta, FontId::proportional(12.0), weak, META_W - 10.0);

    let summary = shown_summary(item);
    let w = line(ui, pos2(x, cy), summary, FontId::proportional(13.5), text, meta_x - x - 16.0);
    if dropped && w > 0.0 {
        p.line_segment([pos2(x, cy + 0.5), pos2(x + w, cy + 0.5)], Stroke::new(1.0_f32, text));
    }
}

/// The summary, or the new first line of a reword with a typed message.
fn shown_summary(item: &TodoItem) -> &str {
    match &item.new_message {
        Some(m) if item.action == RebaseAction::Reword => m.lines().next().unwrap_or_default(),
        _ => &item.summary,
    }
}

/// The action as a painted label in place of its combo box (while rows move).
fn paint_action(ui: &Ui, row: Rect, a: RebaseAction) {
    let rect = Rect::from_min_size(pos2(row.left() + COMBO_X, row.center().y - 11.0), vec2(COMBO_W, 22.0));
    ui.painter().rect_filled(rect, CornerRadius::same(3), color::toolbar());
    let pos = pos2(rect.left() + 8.0, rect.center().y);
    ui.painter().text(pos, Align2::LEFT_CENTER, a.keyword(), FontId::proportional(13.0), action_color(a));
}

/// One line of text cut to `max_w` with an ellipsis, vertically centred on `pos.y`. Returns its
/// width.
fn line(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, c: Color32, max_w: f32) -> f32 {
    if max_w <= 0.0 {
        return 0.0;
    }
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, c);
    job.wrap = TextWrapping::truncate_at_width(max_w);
    let galley = ui.fonts(|f| f.layout_job(job));
    let w = galley.size().x;
    ui.painter().galley(pos2(pos.x, pos.y - galley.size().y / 2.0), galley, c);
    w
}

/// The action combo box and the move buttons of row `i` of `n`.
fn controls(ui: &mut Ui, rect: Rect, i: usize, n: usize, item: &TodoItem, enabled: bool, msgs: &mut Vec<Msg>) {
    let combo_rect = Rect::from_min_size(pos2(rect.left() + COMBO_X, rect.top()), vec2(COMBO_W, ROW_H));
    let mut child = ui.new_child(UiBuilder::new().max_rect(combo_rect).layout(Layout::left_to_right(Align::Center)));
    if !enabled {
        child.disable();
    }
    let mut action = item.action;
    egui::ComboBox::from_id_salt(("rebase-action", item.oid)).width(COMBO_W - 8.0).selected_text(action_text(action)).show_ui(
        &mut child,
        |ui| {
            for a in RebaseAction::ALL {
                ui.selectable_value(&mut action, a, action_text(a));
            }
        },
    );
    if action != item.action {
        msgs.push(Msg::RebaseSetAction { index: i, action });
    }

    let buttons = Rect::from_min_max(pos2(rect.right() - BUTTONS_W, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
    let mut child = ui.new_child(UiBuilder::new().max_rect(buttons).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = 3.0;
    if !enabled {
        child.disable();
    }
    let up = child.add_enabled_ui(i > 0, |ui| icon_button(ui, Icon::PrevChange, "Move up")).inner;
    if up.clicked() {
        msgs.push(Msg::RebaseMove { from: i, to: i - 1 });
    }
    let down = child.add_enabled_ui(i + 1 < n, |ui| icon_button(ui, Icon::NextChange, "Move down")).inner;
    if down.clicked() {
        msgs.push(Msg::RebaseMove { from: i, to: i + 1 });
    }
}

/// The message editor under the row being reworded.
fn editor(ui: &mut Ui, panel: &mut RebasePanel, cache: &mut UiCache, spell: bool, msgs: &mut Vec<Msg>) {
    let enabled = panel.editable();
    // Under the SHA and summary, as wide as the row.
    let width = ui.available_width() - 2.0 * MARGIN - SHA_X;
    ui.horizontal(|ui| {
        ui.add_space(MARGIN + SHA_X);
        ui.vertical(|ui| {
            ui.set_width(width);
            egui::Frame::new()
                .fill(color::panel())
                .stroke(Stroke::new(1.0_f32, color::border()))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.label(RichText::new("New commit message").size(12.0).color(color::weak()));
                        spell_text_edit(ui, &mut panel.draft, "Commit message", 5, &mut cache.speller, spell);
                        ui.horizontal(|ui| {
                            if ui.button("Update message").clicked() {
                                msgs.push(Msg::RebaseSaveMessage);
                            }
                            if ui.link("Cancel").clicked() {
                                msgs.push(Msg::RebaseCancelMessage);
                            }
                        });
                    });
                });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::drop_slot;

    #[test]
    fn drop_slot_rounds_and_clamps() {
        assert_eq!(drop_slot(-50.0, 100.0, 42.0, 4), 0);
        assert_eq!(drop_slot(100.0 + 20.0, 100.0, 42.0, 4), 0);
        assert_eq!(drop_slot(100.0 + 22.0, 100.0, 42.0, 4), 1);
        assert_eq!(drop_slot(100.0 + 84.0, 100.0, 42.0, 4), 2);
        assert_eq!(drop_slot(5000.0, 100.0, 42.0, 4), 3);
    }
}
