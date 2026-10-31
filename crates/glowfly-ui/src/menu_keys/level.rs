//! The keys in one menu level, after its contents are drawn (a submenu's level runs inside its
//! parent's contents, so before the parent's: the innermost level sees a key first).
//!
//! With the focus on one of its items: Up and Down go to the row above or below (wrapping),
//! Home and End to the first and last row, Right and Left between the parts of a row (a split
//! row's button, a repository row's star and ×), Left on a row's first part or Esc closes the
//! level (`close`), and a typed character jumps to the next item whose label starts with it.
//! egui's own arrow moves are locked out on the focused item (they could land on a widget
//! behind the menu). In a focused text field only Up, Down and Esc are the menu's. With nothing
//! focused in any menu, Down and Up focus the first and last item and Esc closes the level. A
//! level that opened from the keyboard focuses its first item once it can.

use super::Kind;
use super::items::{self, Item};
use super::memory::{self, Level, Nav};
use eframe::egui::{self, Event, EventFilter, Id, Key, Modifiers, Order, Ui};

/// Handles this frame's keys in the level `ui` draws, its items being `rows`.
/// `opener` is what had the focus or was named as the opener as the contents began.
pub(super) fn keys(ui: &mut Ui, kind: Kind, rows: &[Vec<Item>], opener: Option<Id>) {
    let ctx = ui.ctx().clone();
    let layer = ui.layer_id();
    let mut nav = memory::load(&ctx);
    let depth = nav.depth.saturating_sub(1);
    let focused = ctx.memory(|m| m.focused());
    let mut level = this_level(&ctx, &mut nav, layer, kind, opener);
    let outside = focused.is_none_or(|f| layer_of(&ctx, f).is_none_or(|l| l.order != Order::Foreground));
    let here = focused.filter(|&f| layer_of(&ctx, f) == Some(layer));
    if level.first || level.last {
        // A menu's first frame only sizes it (its items can't take focus yet), and egui would
        // carry the focus on in an arrow's direction: a later frame does it then.
        if ui.is_sizing_pass() || arrow_pressed(&ctx) {
            ctx.request_repaint();
        } else {
            let to = if level.first { rows.first() } else { rows.last() };
            (level.first, level.last) = (false, false);
            if let Some(row) = to {
                focus(&ctx, row[0].id);
            }
        }
    }
    if let Some(f) = here {
        level.last_focus = Some(f);
        focused_keys(ui, &mut nav, &mut level, depth, rows, f);
    } else if outside {
        if take(&ctx, Key::ArrowDown) {
            level.first = true;
            ctx.request_repaint();
        } else if take(&ctx, Key::ArrowUp) {
            level.last = true;
            ctx.request_repaint();
        } else if take(&ctx, Key::Escape) {
            close(ui, &mut nav, &level, true);
        }
    }
    save(&mut nav, level);
    memory::store(&ctx, nav);
}

/// This pass's record of the level on `layer`: carried over from the last pass, or new (then
/// it remembers what opened it, and opened from the keyboard it focuses its first item).
fn this_level(ctx: &egui::Context, nav: &mut Nav, layer: egui::LayerId, kind: Kind, opener: Option<Id>) -> Level {
    let pending = nav.pending.iter().position(|&(_, l)| l == layer).map(|i| nav.pending.remove(i)).is_some();
    let mut level = match memory::level_of(nav, layer) {
        Some(l) => Level { kind, ..l.clone() },
        None => {
            let invoker = match kind {
                Kind::Native { row } | Kind::Keyed { row } => Some(row),
                Kind::Root | Kind::Popup { .. } => opener.filter(|&f| layer_of(ctx, f) != Some(layer)),
            };
            Level { layer, kind, last_focus: None, invoker, first: nav.keyboard, last: false }
        }
    };
    level.first |= pending;
    level
}

fn save(nav: &mut Nav, level: Level) {
    match nav.levels.iter_mut().find(|l| l.layer == level.layer) {
        Some(l) => *l = level,
        None => nav.levels.push(level),
    }
}

/// The keys while `f`, an item of this level (or another widget on its layer), has the focus.
fn focused_keys(ui: &mut Ui, nav: &mut Nav, level: &mut Level, depth: usize, rows: &[Vec<Item>], f: Id) {
    let ctx = ui.ctx().clone();
    let text = egui::text_edit::TextEditState::load(&ctx, f).is_some();
    if !text {
        let filter = EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() };
        ctx.memory_mut(|m| m.set_focus_lock_filter(f, filter));
    }
    let at = items::find(rows, f);
    let n = rows.len();
    let row_start = |r: usize| rows[r][0].id;
    let to = if n == 0 {
        None
    } else if take(&ctx, Key::ArrowDown) {
        Some(row_start(at.map_or(0, |(r, _)| (r + 1) % n)))
    } else if take(&ctx, Key::ArrowUp) {
        Some(row_start(at.map_or(n - 1, |(r, _)| (r + n - 1) % n)))
    } else if !text && take(&ctx, Key::Home) {
        Some(row_start(0))
    } else if !text && take(&ctx, Key::End) {
        Some(row_start(n - 1))
    } else {
        None
    };
    if let Some(to) = to {
        focus(&ctx, to);
        return;
    }
    if take(&ctx, Key::Escape) {
        close(ui, nav, level, true);
        return;
    }
    if text {
        return;
    }
    if let Some((r, p)) = at {
        if p + 1 < rows[r].len() && take(&ctx, Key::ArrowRight) {
            focus(&ctx, rows[r][p + 1].id);
            return;
        }
        if p > 0 && take(&ctx, Key::ArrowLeft) {
            focus(&ctx, rows[r][p - 1].id);
            return;
        }
    }
    // Left on a row closes a submenu; in a root menu the bar may use it.
    let sub = matches!(level.kind, Kind::Native { .. } | Kind::Keyed { .. }) || depth > 0;
    if sub && take(&ctx, Key::ArrowLeft) {
        close(ui, nav, level, false);
        return;
    }
    if let Some(c) = typed(&ctx)
        && let Some(to) = items::typeahead(rows, Some(f), c)
    {
        focus(&ctx, to);
    }
    if let Kind::Popup { close_on_click: true } = level.kind {
        // egui closes such a popup on a click, not on Enter or Space.
        let item = at.map(|(r, p)| &rows[r][p]);
        let activated = ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space));
        if activated && item.is_some_and(|i| !i.opens && ctx.read_response(i.id).is_some_and(|r| r.sense.senses_click())) {
            ctx.memory_mut(|m| m.close_popup());
        }
    }
}

/// Closes the level (`esc`: by Esc, else Left): a root menu or popup closes and the focus goes
/// back to what opened it; a submenu's row closes it (`super::take_close`) and takes the focus.
fn close(ui: &mut Ui, nav: &mut Nav, level: &Level, esc: bool) {
    let ctx = ui.ctx().clone();
    // The next frame draws it closed.
    ctx.request_repaint();
    match level.kind {
        Kind::Root => ui.close_menu(),
        Kind::Popup { .. } => ctx.memory_mut(|m| m.close_popup()),
        Kind::Keyed { .. } | Kind::Native { .. } => nav.close.push((level.layer, esc)),
    }
    if matches!(level.kind, Kind::Root | Kind::Popup { .. }) && nav.depth <= 1 {
        nav.closed_pass = nav.pass;
    }
    match level.invoker {
        // egui's submenu: its row decides (`menu_submenu`).
        _ if matches!(level.kind, Kind::Native { .. }) => {}
        Some(back) => focus(&ctx, back),
        None => ctx.memory_mut(|m| {
            if let Some(f) = m.focused() {
                m.surrender_focus(f);
            }
        }),
    }
}

/// The first single printable character typed this frame (not a space), taken.
fn typed(ctx: &egui::Context) -> Option<char> {
    ctx.input_mut(|i| {
        let at = i.events.iter().position(|e| matches!(e, Event::Text(s) if single(s).is_some()))?;
        let Event::Text(s) = i.events.remove(at) else { return None };
        single(&s)
    })
}

fn single(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() && !c.is_control() => Some(c),
        _ => None,
    }
}

/// Takes a press of `key` alone.
fn take(ctx: &egui::Context, key: Key) -> bool {
    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key))
}

pub(super) fn layer_of(ctx: &egui::Context, id: Id) -> Option<egui::LayerId> {
    ctx.read_response(id).map(|r| r.layer_id)
}

/// Focuses `id`, with a frame after it so the item takes the arrows from egui before the next key.
pub(super) fn focus(ctx: &egui::Context, id: Id) {
    ctx.memory_mut(|m| m.request_focus(id));
    ctx.request_repaint();
}

/// Whether an arrow key was pressed this frame (taken or not).
fn arrow_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        i.raw
            .events
            .iter()
            .any(|e| matches!(e, Event::Key { key: Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight, pressed: true, .. }))
    })
}
