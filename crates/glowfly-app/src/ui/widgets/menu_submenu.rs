//! When a menu's submenu opens and closes, and where it goes (`menu::submenu`). egui 0.31 opens
//! a submenu the moment the pointer touches its row, always right of the menu, and its state is
//! private, so this steers egui's own submenu from outside, with the row in a child `Ui` of its
//! own:
//!
//! - **Open delay.** Until the pointer has rested `OPEN_DELAY` on the row (or clicked it, or
//!   pressed Enter, Space or Right on it), a click-sensing cover over the row takes the hover, so
//!   egui doesn't see its button hovered and doesn't open; the row is drawn hovered meanwhile.
//! - **Close grace.** egui closes an open submenu when the pointer is elsewhere in the `Ui` that
//!   holds its row. That `Ui` is the row's child, which never holds the pointer away from the
//!   row, so egui never closes it by itself (another row opening its submenu still replaces it,
//!   and egui's "moving towards the submenu" check still runs). After the pointer has been off
//!   both the row and the submenu for `CLOSE_GRACE`, somewhere else in the menu, the child is
//!   grown over the menu for a frame, and egui closes the submenu the next.
//! - **Placement.** egui places a submenu at `menu.right + menu_spacing` and `row.top -
//!   menu frame's top margin`, read from the row's `Ui`, so the child's `menu_spacing` and frame
//!   stroke width put it where `place` says: right of the menu, or left when it doesn't fit, and
//!   shifted only as needed to stay `EDGE` inside the window. That needs the submenu's size,
//!   known once it has been shown. The first time, it opens where egui puts it (right of the
//!   menu, so never under the pointer), egui's first frame of it (invisible, sizing) measures
//!   it, and if it belongs elsewhere the cover closes it before it is drawn and it opens again
//!   in its place two frames later.
//!
//! One level only: a submenu's own submenu rows are rows of the same kind.

use eframe::egui::{self, Id, InnerResponse, Key, LayerId, Pos2, Rect, Sense, Ui, UiBuilder, UiKind, Vec2, WidgetText, pos2};
use std::cell::Cell;

/// How long the pointer rests on a row before its submenu opens, in seconds.
pub const OPEN_DELAY: f64 = 0.25;
/// How long a submenu stays open after the pointer leaves both its row and itself, in seconds.
pub const CLOSE_GRACE: f64 = 0.4;
/// The least distance between a submenu and the window's edges.
pub const EDGE: f32 = 8.0;

/// What a submenu row keeps between frames.
#[derive(Clone, Copy, Debug, Default)]
struct Row {
    /// The row's rect last frame.
    rect: Option<Rect>,
    /// When the pointer came to rest on the row (it is still there).
    since: Option<f64>,
    /// May egui open it: the delay is over, or it was clicked or opened with a key.
    armed: bool,
    /// The pass it was armed in.
    armed_pass: u64,
    /// Whether its submenu showed last frame.
    open: bool,
    /// When the pointer left both the row and the open submenu.
    left_at: Option<f64>,
    /// The open submenu's layer and size, once shown.
    layer: Option<LayerId>,
    size: Option<Vec2>,
    /// Closing it, the first time, to open it again where its measured size says.
    reopen: bool,
}

/// A submenu row: inside a menu with the open delay, close grace and placement above; outside
/// one, egui's menu button.
pub(super) fn show<R>(ui: &mut Ui, title: WidgetText, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    if !ui.stack().iter().any(|s| s.kind() == Some(UiKind::Menu)) {
        return ui.menu_button(title, add_contents);
    }
    let key = ui.id().with(("glowfly-submenu", title.text().to_owned()));
    let ctx = ui.ctx().clone();
    let mut row = ctx.data(|d| d.get_temp::<Row>(key)).unwrap_or_default();
    let now = ui.input(|i| i.time);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let menu = ctx.memory(|m| m.area_rect(ui.layer_id().id));
    let on_row = row.rect.is_some_and(|r| ui.rect_contains_pointer(r));
    if row.reopen && !row.open {
        row.reopen = false;
    }
    // Opening it again needs the pointer on the row; without, it stays where it opened.
    row.reopen &= on_row;
    let on_sub = row.open && pointer.zip(row.layer).is_some_and(|(p, l)| ctx.layer_id_at(p) == Some(l));
    intent(&ctx, &mut row, on_row, now);
    let close = row.open && grace_over(&ctx, &mut row, on_row || on_sub, now) && pointer.zip(menu).is_some_and(|(p, m)| m.contains(p));

    let area = ui.available_rect_before_wrap();
    // The child takes this auto id's place in `ui`, and no widget gets it (`row_at`).
    let slot = ui.next_auto_id();
    let mut child = ui.new_child(UiBuilder::new().id_salt(key).max_rect(area));
    let waiting = !row.armed && !row.open;
    if waiting && on_row {
        // The cover takes the hover from egui's button; draw the row hovered all the same.
        let w = &mut child.visuals_mut().widgets;
        w.inactive = w.hovered;
    }
    if let (Some(m), Some(size)) = (menu, row.size) {
        aim(&mut child, m, area.top(), size);
    }
    let layer = Cell::new(None);
    let inner = child.menu_button(title, |ui| {
        layer.set(Some(ui.layer_id()));
        add_contents(ui)
    });
    let rect = inner.response.rect;
    let open = inner.inner.is_some();
    if open {
        row.layer = layer.get().or(row.layer);
        let size = row.layer.and_then(|l| ctx.memory(|m| m.area_rect(l.id))).map(|r| r.size());
        if let (Some(size), Some(m), None) = (size, menu, row.size) {
            // Opened unaimed: egui's spot, unless `placement` puts it elsewhere.
            let margin_top = egui::Frame::menu(child.style()).total_margin().top;
            let gap = child.spacing().menu_spacing;
            let egui_at = pos2(m.right() + gap, area.top() - margin_top);
            let at = placement(m, egui_at.y, size, ctx.screen_rect(), gap.max(0.0));
            if (at - egui_at).length() > 0.5 && on_row {
                row.reopen = true;
                arm(&ctx, &mut row);
            }
        }
        row.size = size.or(row.size);
    }
    if waiting || row.reopen {
        let cover = child.interact(rect, key.with("cover"), Sense::CLICK);
        if cover.clicked() {
            arm(&ctx, &mut row);
        }
    }
    if let Some(m) = menu.filter(|_| close) {
        // egui closes it next frame, finding the pointer in the row's `Ui` but not on the row.
        child.expand_to_include_rect(m);
        ctx.request_repaint();
    }
    drop(child);
    ui.advance_cursor_after_rect(rect);
    row.rect = Some(rect);
    row.open = open;
    if !open {
        row.left_at = None;
    }
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        d.insert_temp(key, row);
        d.insert_temp(slot.with(SLOT), (pass, inner.response.id));
    });
    inner
}

const SLOT: &str = "glowfly-submenu-row";

/// The widget at the auto id `id` of a menu's `Ui`: `id` itself, or for a submenu row drawn
/// this pass (in a child `Ui` that took that id's place) the row's button. For code that walks
/// a menu's items by their auto ids after drawing them (`menu_bar::nav`).
pub fn row_at(ctx: &egui::Context, id: Id) -> Id {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, Id)>(id.with(SLOT))).filter(|&(p, _)| p == pass).map_or(id, |(_, row)| row)
}

/// Tracks the pointer resting on the row, and arms it when the delay is over or a key asks.
fn intent(ctx: &egui::Context, row: &mut Row, on_row: bool, now: f64) {
    if !on_row {
        row.since = None;
        row.armed = false;
        return;
    }
    let since = *row.since.get_or_insert(now);
    if row.open {
        return;
    }
    if row.armed {
        // egui opens it two passes after arming (the cover's last hover goes first); keep them
        // coming, but not forever if egui doesn't.
        if ctx.cumulative_pass_nr() <= row.armed_pass + 3 {
            ctx.request_repaint();
        }
        return;
    }
    let key = ctx.input(|i| [Key::Enter, Key::Space, Key::ArrowRight].into_iter().any(|k| i.key_pressed(k)));
    let waited = now - since;
    if key || waited >= OPEN_DELAY {
        arm(ctx, row);
    } else {
        ctx.request_repaint_after_secs((OPEN_DELAY - waited) as f32);
    }
}

/// Lets egui open the row's submenu: next frame the cover is gone, the frame after its button
/// is hovered.
fn arm(ctx: &egui::Context, row: &mut Row) {
    row.armed = true;
    row.armed_pass = ctx.cumulative_pass_nr();
    ctx.request_repaint();
}

/// Whether the open submenu's grace is over: the pointer has been off the row and the submenu
/// for `CLOSE_GRACE`.
fn grace_over(ctx: &egui::Context, row: &mut Row, inside: bool, now: f64) -> bool {
    if inside {
        row.left_at = None;
        return false;
    }
    let waited = now - *row.left_at.get_or_insert(now);
    if waited < CLOSE_GRACE {
        ctx.request_repaint_after_secs((CLOSE_GRACE - waited) as f32);
    }
    waited >= CLOSE_GRACE
}

/// Sets `child`'s menu spacing and frame stroke, which egui reads to place the submenu of the
/// row at `row_top`, so it opens where `placement` says.
fn aim(child: &mut Ui, menu: Rect, row_top: f32, size: Vec2) {
    let margin_top = egui::Frame::menu(child.style()).total_margin().top;
    let gap = child.spacing().menu_spacing.max(0.0);
    let at = placement(menu, row_top - margin_top, size, child.ctx().screen_rect(), gap);
    let style = child.style_mut();
    style.spacing.menu_spacing = at.x - menu.right();
    // egui subtracts the frame's top margin (inner margin plus stroke) from the row's top.
    style.visuals.window_stroke.width = row_top - at.y - f32::from(style.spacing.menu_margin.top);
}

/// Where a submenu of `size` opens for the menu `menu`, its first row level with the parent
/// row (`top` is that alignment's top edge): right of the menu `gap` away, or left of it when
/// it doesn't fit on the right but does on the left (else on the roomier side), then moved only
/// as far as it takes to stay `EDGE` inside `screen`.
pub(super) fn placement(menu: Rect, top: f32, size: Vec2, screen: Rect, gap: f32) -> Pos2 {
    let (inner_l, inner_r) = (screen.left() + EDGE, screen.right() - EDGE);
    let right = menu.right() + gap;
    let left = menu.left() - gap - size.x;
    let x = if right + size.x <= inner_r || (left < inner_l && inner_r - menu.right() >= menu.left() - inner_l) { right } else { left };
    let x = x.min(inner_r - size.x).max(inner_l);
    let y = top.min(screen.bottom() - EDGE - size.y).max(screen.top() + EDGE);
    pos2(x, y)
}

#[cfg(test)]
#[path = "menu_submenu_tests.rs"]
mod tests;
