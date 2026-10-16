//! The tab bar above the toolbar: one tab per open repository, plus a "new tab" button. Tabs
//! are dragged sideways to reorder them; right-clicking one offers the close actions.

use super::theme::color;
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::AppState;
use crate::state::tabs::index_after_move;
use eframe::egui::{self, Align2, Button, CursorIcon, FontId, Id, PointerButton, Rect, Response, Sense, Stroke, StrokeKind, pos2, vec2};

const HEIGHT: f32 = 30.0;
const ROUNDING: egui::CornerRadius = egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 };

/// A tab being dragged: its index and where on it the pointer grabbed it (from its left edge).
/// Kept in egui's temporary memory, since the press position is gone by the release frame.
#[derive(Clone, Copy)]
struct TabDrag {
    from: usize,
    grab: f32,
}

/// A drag in progress: the dragged tab, the slot it would drop into, and where it is drawn.
#[derive(Clone, Copy)]
struct Floating {
    from: usize,
    to: usize,
    rect: Rect,
}

/// One tab's place in the strip and its interactions this frame.
struct Slot {
    rect: Rect,
    r: Response,
    close: Response,
}

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    egui::TopBottomPanel::top("tabs")
        .exact_height(HEIGHT)
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(egui::Margin { left: 6, right: 6, top: 0, bottom: 0 }))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                // Tabs shrink to share the width once there are many.
                let room = ui.available_width() - HEIGHT;
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
                let floating = drag(ui, &slots, strip_left, pitch, msgs);
                // The active tab's name comes from its loaded repository.
                let name = |i: usize| match (&st.snap, i == st.active_tab) {
                    (Some(snap), true) => snap.name.as_str(),
                    _ => st.tabs[i].name.as_str(),
                };
                for (i, Slot { rect, r, close }) in slots.into_iter().enumerate() {
                    let active = i == st.active_tab;
                    let p = ui.painter();
                    match floating {
                        Some(f) if f.from == i => {}
                        // While dragging, the others slide to where they would be after the drop.
                        Some(f) => {
                            let shift = (index_after_move(i, f.from, f.to) as f32 - i as f32) * pitch;
                            paint_tab(p, rect.translate(vec2(shift, 0.0)), name(i), active, false, false, false);
                        }
                        None => {
                            let (hovered, close_hovered) = (r.hovered(), close.hovered());
                            paint_tab(p, rect, name(i), active, hovered, active || hovered || close_hovered, close_hovered);
                            let r = match &st.tabs[i].path {
                                Some(path) => r.on_hover_text(trellis_core::native_path::shown(path)),
                                None => r,
                            };
                            if close.clicked() || r.middle_clicked() {
                                msgs.push(Msg::Tabs(TabsMsg::CloseTab(i)));
                            } else if r.clicked() {
                                msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
                            }
                            r.context_menu(|ui| tab_menu(ui, i, st, msgs));
                        }
                    }
                }
                // The dragged tab, drawn last so it floats above the others.
                if let Some(f) = floating {
                    let active = f.from == st.active_tab;
                    paint_tab(ui.painter(), f.rect, name(f.from), active, false, active, false);
                    ui.painter().rect_stroke(f.rect, ROUNDING, Stroke::new(1.0_f32, color::primary()), StrokeKind::Inside);
                }
                new_tab_button(ui, msgs);
            });
        });
}

/// Tracks a tab drag: returns where the dragged tab floats while one is in progress, and pushes
/// `MoveTab` when it is dropped in another slot.
fn drag(ui: &egui::Ui, slots: &[Slot], strip_left: f32, pitch: f32, msgs: &mut Vec<Msg>) -> Option<Floating> {
    let drag_id = Id::new("tab-drag");
    let n = slots.len();
    if n < 2 {
        return None;
    }
    if let Some((i, s)) = slots.iter().enumerate().find(|(_, s)| s.r.drag_started_by(PointerButton::Primary))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(drag_id, TabDrag { from: i, grab: origin.x - s.rect.left() }));
    }
    let state = ui.data(|d| d.get_temp::<TabDrag>(drag_id))?;
    let slot = slots.get(state.from).filter(|s| s.r.dragged() || s.r.drag_stopped());
    let (Some(slot), Some(px)) = (slot, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this tab (e.g. the tabs changed under it).
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        return None;
    };
    let left = (px.x - state.grab).clamp(strip_left, strip_left + (n - 1) as f32 * pitch);
    let to = drop_slot(left, strip_left, pitch, n);
    if slot.r.drag_stopped() {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        if to != state.from {
            msgs.push(Msg::Tabs(TabsMsg::MoveTab { from: state.from, to }));
        }
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Floating { from: state.from, to, rect: Rect::from_min_size(pos2(left, slot.rect.top()), slot.rect.size()) })
}

/// The slot nearest to a tab whose left edge is at `left`.
fn drop_slot(left: f32, strip_left: f32, pitch: f32, n: usize) -> usize {
    (((left - strip_left) / pitch).round().max(0.0) as usize).min(n - 1)
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
    p.with_clip_rect(clip).text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(13.0), text_col);
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

fn new_tab_button(ui: &mut egui::Ui, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(HEIGHT, HEIGHT), Sense::click());
    let r = r.on_hover_text(format!("New tab ({})", crate::keymap::hint(crate::keymap::BindingId::NewTab)));
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
fn tab_menu(ui: &mut egui::Ui, i: usize, st: &AppState, msgs: &mut Vec<Msg>) {
    let n = st.tabs.len();
    item(ui, msgs, true, Button::new("Close tab"), Msg::Tabs(TabsMsg::CloseTab(i)));
    item(ui, msgs, n > 1, Button::new("Close other tabs"), Msg::Tabs(TabsMsg::CloseOtherTabs(i)));
    item(ui, msgs, i + 1 < n, Button::new("Close tabs to the right"), Msg::Tabs(TabsMsg::CloseTabsToRight(i)));
    ui.separator();
    let reopen = Button::new("Reopen closed tab").shortcut_text(crate::keymap::hint(crate::keymap::BindingId::ReopenTab));
    let r = item(ui, msgs, !st.closed_tabs.is_empty(), reopen, Msg::Tabs(TabsMsg::ReopenClosedTab));
    if let Some(path) = st.closed_tabs.last() {
        r.on_hover_text(trellis_core::native_path::shown(path));
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
