//! The tab bar above the toolbar: one tab per open repository, plus a "new tab" button and, at
//! the far end when there are closed tabs or the tabs had to shrink, the tab list
//! (`tab_list.rs`). Tabs are dragged sideways to reorder them (Esc cancels a drag);
//! right-clicking one offers Rename tab and the close actions. With Group worktree tabs on, tabs
//! of one repository's worktrees form a group (`state/tab_groups.rs`) marked by a colored strip
//! along their bottom; a tab dropped outside its group takes the group.

use super::theme::color;
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::AppState;
use crate::state::menu_bar::tab_name;
use crate::state::tab_groups::{self, is_identity, move_order, new_index, shown_at};
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Align, Align2, Button, Color32, CursorIcon, Id, Key, Layout, PointerButton, Rect, Response, Sense, Stroke, StrokeKind, pos2, vec2,
};
use std::ops::Range;
use trellis_core::i18n::Lang;

pub(super) const HEIGHT: f32 = 30.0;
const ROUNDING: egui::CornerRadius = egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 };
/// Where the drag in progress is kept in egui's temporary memory.
const DRAG_ID: &str = "tab-drag";

/// A tab being dragged: its index and where on it the pointer grabbed it (from its left edge).
/// Kept in egui's temporary memory, since the press position is gone by the release frame.
/// `cancelled` once Esc snapped it back: the rest of that drag does nothing.
#[derive(Clone, Copy)]
struct TabDrag {
    from: usize,
    grab: f32,
    cancelled: bool,
}

/// Whether a tab is being dragged now (and the drag wasn't cancelled): Esc then cancels the
/// drag and nothing else.
pub(crate) fn dragging(ctx: &egui::Context) -> bool {
    let drag = ctx.data(|d| d.get_temp::<TabDrag>(Id::new(DRAG_ID)));
    drag.is_some_and(|d| !d.cancelled) && ctx.input(|i| i.pointer.primary_down())
}

/// A drag in progress: the dragged tab, the order a drop now would give (`order[new] == old`),
/// and where it is drawn.
struct Floating {
    from: usize,
    order: Vec<usize>,
    rect: Rect,
}

/// One tab's place in the strip and its interactions this frame.
struct Slot {
    rect: Rect,
    r: Response,
    close: Response,
}

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    egui::TopBottomPanel::top("tabs")
        .exact_height(HEIGHT)
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(egui::Margin { left: 6, right: 6, top: 0, bottom: 0 }))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                // Tabs shrink to share the width once there are many.
                let list = super::tab_list::wanted(st, ui.available_width());
                let room = ui.available_width() - HEIGHT * (1.0 + f32::from(u8::from(list)));
                let width = (room / st.tabs.len() as f32).clamp(60.0, 220.0);
                let pitch = width + ui.spacing().item_spacing.x;
                let strip_left = ui.cursor().left();
                let slots: Vec<Slot> = (0..st.tabs.len())
                    .map(|i| {
                        let (rect, _) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::hover());
                        let r = ui.interact(rect, ui.id().with(("tab", i)), Sense::click_and_drag());
                        let close = ui.interact(close_rect(rect), r.id.with("close"), Sense::click());
                        Slot { rect, r, close }
                    })
                    .collect();
                let on = st.settings.group_worktree_tabs;
                let blocks = tab_groups::blocks(&st.tabs, on);
                let shown = tab_groups::shown_groups(&st.tabs, on);
                let floating = drag(ui, &slots, &blocks, strip_left, pitch, msgs);
                let gap = pitch - width;
                let name = |i: usize| tab_name(st, i);
                for (i, Slot { rect, r, close }) in slots.into_iter().enumerate() {
                    let active = i == st.active_tab;
                    let p = ui.painter();
                    let group = shown_at(&shown, i);
                    match &floating {
                        Some(f) if f.from == i => {}
                        // While dragging, the others slide to where they would be after the drop.
                        Some(f) => {
                            let shift = (new_index(&f.order, i) as f32 - i as f32) * pitch;
                            let rect = rect.translate(vec2(shift, 0.0));
                            paint_tab(p, rect, name(i), active, false, false, false);
                            if let Some((members, n)) = group {
                                // Where the group's ends will be after the drop.
                                let at = |j: usize| new_index(&f.order, j);
                                let (lo, hi) = members.clone().map(at).fold((usize::MAX, 0), |(lo, hi), j| (lo.min(j), hi.max(j)));
                                paint_group_strip(p, rect, group_color(*n), at(i) == lo, at(i) == hi, gap);
                            }
                        }
                        None => {
                            let (hovered, close_hovered) = (r.hovered(), close.hovered());
                            paint_tab(p, rect, name(i), active, hovered, active || hovered || close_hovered, close_hovered);
                            if let Some((members, n)) = group {
                                paint_group_strip(p, rect, group_color(*n), i == members.start, i + 1 == members.end, gap);
                            }
                            let r = match (&st.tabs[i].path, group.and(st.tabs[i].group.as_ref())) {
                                (Some(path), Some(g)) => {
                                    let shown = trellis_core::native_path::shown(path);
                                    r.on_hover_text(t.trf("tabs.group_tip", &[("path", &shown), ("group", &g.name)]))
                                }
                                (Some(path), None) => r.on_hover_text(trellis_core::native_path::shown(path)),
                                (None, _) => r,
                            };
                            if close.clicked() || r.middle_clicked() {
                                msgs.push(Msg::Tabs(TabsMsg::CloseTab(i)));
                            } else if r.clicked() {
                                msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
                            }
                            r.context_menu(|ui| tab_menu(ui, t, i, st, msgs));
                        }
                    }
                }
                // The dragged tab, drawn last so it floats above the others.
                if let Some(f) = floating {
                    let active = f.from == st.active_tab;
                    paint_tab(ui.painter(), f.rect, name(f.from), active, false, active, false);
                    if let Some((_, n)) = shown_at(&shown, f.from) {
                        paint_group_strip(ui.painter(), f.rect, group_color(*n), true, true, gap);
                    }
                    ui.painter().rect_stroke(f.rect, ROUNDING, Stroke::new(1.0_f32, color::primary()), StrokeKind::Inside);
                }
                new_tab_button(ui, t, &st.keymap, msgs);
                if list {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| super::tab_list::button(ui, t, st, msgs));
                }
            });
        });
}

/// Tracks a tab drag: returns where the dragged tab floats while one is in progress, and pushes
/// `MoveTab` with the slot under it when dropping it there changes the order (`blocks` are the
/// tabs that move together, `tab_groups::move_order`).
fn drag(ui: &egui::Ui, slots: &[Slot], blocks: &[Range<usize>], strip_left: f32, pitch: f32, msgs: &mut Vec<Msg>) -> Option<Floating> {
    let drag_id = Id::new(DRAG_ID);
    let n = slots.len();
    if n < 2 {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        return None;
    }
    if let Some((i, s)) = slots.iter().enumerate().find(|(_, s)| s.r.drag_started_by(PointerButton::Primary))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(drag_id, TabDrag { from: i, grab: origin.x - s.rect.left(), cancelled: false }));
    }
    let mut state = ui.data(|d| d.get_temp::<TabDrag>(drag_id))?;
    let slot = slots.get(state.from).filter(|s| s.r.dragged() || s.r.drag_stopped());
    let (Some(slot), Some(px)) = (slot, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this tab (e.g. the tabs changed under it).
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        return None;
    };
    if !state.cancelled && ui.input(|i| i.key_pressed(Key::Escape)) {
        state.cancelled = true;
        ui.data_mut(|d| d.insert_temp(drag_id, state));
    }
    if state.cancelled {
        // Snapped back: the tab stays in its slot until the button is released.
        if slot.r.drag_stopped() {
            ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        }
        return None;
    }
    let left = (px.x - state.grab).clamp(strip_left, strip_left + (n - 1) as f32 * pitch);
    let to = drop_slot(left, strip_left, pitch, n);
    let order = move_order(blocks, state.from, to);
    if slot.r.drag_stopped() {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        if !is_identity(&order) {
            msgs.push(Msg::Tabs(TabsMsg::MoveTab { from: state.from, to }));
        }
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Floating { from: state.from, order, rect: Rect::from_min_size(pos2(left, slot.rect.top()), slot.rect.size()) })
}

/// The slot nearest to a tab whose left edge is at `left`.
fn drop_slot(left: f32, strip_left: f32, pitch: f32, n: usize) -> usize {
    (((left - strip_left) / pitch).round().max(0.0) as usize).min(n - 1)
}

/// The color of the `n`th group from the left, so neighbouring groups differ.
fn group_color(n: usize) -> Color32 {
    [color::blue(), color::purple(), color::green(), color::amber()][n % 4].gamma_multiply(0.8)
}

/// The strip under a grouped tab: inset at the group's outer ends, and across the `gap` to the
/// next tab elsewhere, so it runs unbroken along the group.
fn paint_group_strip(p: &egui::Painter, rect: Rect, color: Color32, first: bool, last: bool, gap: f32) {
    let left = rect.left() + if first { 4.0 } else { 0.0 };
    let right = rect.right() + if last { -4.0 } else { gap };
    p.rect_filled(Rect::from_min_max(pos2(left, rect.bottom() - 3.0), pos2(right, rect.bottom() - 1.0)), 1.0, color);
}

fn close_rect(tab: Rect) -> Rect {
    Rect::from_center_size(pos2(tab.right() - 14.0, tab.center().y), vec2(16.0, 16.0))
}

fn paint_tab(p: &egui::Painter, rect: Rect, name: &str, active: bool, hovered: bool, show_close: bool, close_hovered: bool) {
    let close_rect = close_rect(rect);
    let fill = if active {
        color::toolbar()
    } else if hovered {
        color::hover()
    } else {
        color::bg()
    };
    p.rect_filled(rect, ROUNDING, fill);
    if active {
        p.line_segment([rect.left_top() + vec2(6.0, 0.5), rect.right_top() + vec2(-6.0, 0.5)], Stroke::new(2.0_f32, color::primary()));
    }
    // Name, clipped before the close button.
    let text_col = if active { color::text() } else { color::weak() };
    let clip = Rect::from_min_max(rect.min, pos2(close_rect.left() - 4.0, rect.max.y));
    p.with_clip_rect(clip).text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        type_scale::font(type_scale::BODY),
        text_col,
    );
    // Close button: shown on the active and hovered tabs.
    if show_close {
        if close_hovered {
            p.rect_filled(close_rect, 3.0, color::selected());
        }
        let c = close_rect.center();
        let s = Stroke::new(1.4_f32, text_col);
        p.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], s);
        p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], s);
    }
}

fn new_tab_button(ui: &mut egui::Ui, t: Lang, keys: &crate::keymap::Keymap, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(HEIGHT, HEIGHT), Sense::click());
    let r = r.on_hover_text(keys.tip(t.tr("tabs.new_tab.tip"), crate::keymap::BindingId::NewTab));
    let c = rect.center();
    let col = if r.hovered() { color::text() } else { color::weak() };
    let s = Stroke::new(1.6_f32, col);
    ui.painter().line_segment([pos2(c.x - 6.0, c.y), pos2(c.x + 6.0, c.y)], s);
    ui.painter().line_segment([pos2(c.x, c.y - 6.0), pos2(c.x, c.y + 6.0)], s);
    if r.clicked() {
        msgs.push(Msg::Tabs(TabsMsg::NewTab));
    }
}

/// The right-click menu of tab `i`. It acts on that tab without selecting it.
fn tab_menu(ui: &mut egui::Ui, t: Lang, i: usize, st: &AppState, msgs: &mut Vec<Msg>) {
    let n = st.tabs.len();
    item(ui, msgs, st.tabs[i].path.is_some(), Button::new(t.tr("tabs.menu.rename")), Msg::Tabs(TabsMsg::RenameTabDialog(i)));
    ui.separator();
    item(ui, msgs, true, Button::new(t.tr("tabs.menu.close")), Msg::Tabs(TabsMsg::CloseTab(i)));
    item(ui, msgs, n > 1, Button::new(t.tr("tabs.menu.close_others")), Msg::Tabs(TabsMsg::CloseOtherTabs(i)));
    item(ui, msgs, i + 1 < n, Button::new(t.tr("tabs.menu.close_right")), Msg::Tabs(TabsMsg::CloseTabsToRight(i)));
    ui.separator();
    let reopen_key = st.keymap.hint(crate::keymap::BindingId::ReopenTab).unwrap_or_default();
    let reopen = Button::new(t.tr("tabs.menu.reopen")).shortcut_text(reopen_key);
    let r = item(ui, msgs, !st.closed_tabs.is_empty(), reopen, Msg::Tabs(TabsMsg::ReopenClosedTab));
    if let Some(closed) = st.closed_tabs.last() {
        r.on_hover_text(trellis_core::native_path::shown(&closed.path));
    }
}

/// A menu entry, greyed out unless `enabled`; clicking it pushes `msg` and closes the menu.
fn item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, enabled: bool, button: Button, msg: Msg) -> Response {
    let r = ui.add_enabled(enabled, button);
    if r.clicked() {
        msgs.push(msg);
        ui.close_menu();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::drop_slot;
    use crate::state::tabs::index_after_move;

    #[test]
    fn drop_slot_rounds_and_clamps() {
        assert_eq!(drop_slot(-50.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0 + 110.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0 + 111.0, 6.0, 221.0, 3), 1);
        assert_eq!(drop_slot(6.0 + 442.0, 6.0, 221.0, 3), 2);
        assert_eq!(drop_slot(5000.0, 6.0, 221.0, 3), 2);
    }

    #[test]
    fn index_after_move_shifts_the_tabs_in_between() {
        // Moving tab 1 right to 3: tabs 2 and 3 shift left, the others stay.
        assert_eq!([0, 1, 2, 3, 4].map(|i| index_after_move(i, 1, 3)), [0, 3, 1, 2, 4]);
        // Moving tab 3 left to 1: tabs 1 and 2 shift right.
        assert_eq!([0, 1, 2, 3, 4].map(|i| index_after_move(i, 3, 1)), [0, 2, 3, 1, 4]);
        // The moved tab lands at `to`; moving to the same place changes nothing.
        assert_eq!(index_after_move(1, 1, 3), 3);
        assert_eq!([0, 1, 2].map(|i| index_after_move(i, 1, 1)), [0, 1, 2]);
    }
}
