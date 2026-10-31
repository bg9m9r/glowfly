//! Keeping an open menu's keys away from everything behind it. At the start of the frame
//! (`capture`, before the shortcuts run) the keys without Ctrl, Alt or Cmd and the typed text are
//! taken out of the input while a menu is open; the root menu's contents get them back
//! (`restore`), and what its levels leave is set aside again after them (`keep`), so a view drawn
//! later never sees them. The code that draws a menu takes the keys it handles around the menu
//! with `take_key` (the bar's Left and Right, the title bar's Esc).

use super::memory;
use eframe::egui::{self, Event, Key, Modifiers};

/// The events a menu keeps: keys without Ctrl, Alt or Cmd, and typed text.
fn kept(e: &Event) -> bool {
    match e {
        Event::Key { modifiers, .. } => !(modifiers.ctrl || modifiers.alt || modifiers.command || modifiers.mac_cmd),
        Event::Text(_) => true,
        _ => false,
    }
}

/// Takes the kept events out of this frame's input.
fn take_kept(ctx: &egui::Context) -> Vec<Event> {
    ctx.input_mut(|i| {
        let (kept, rest): (Vec<Event>, Vec<Event>) = std::mem::take(&mut i.events).into_iter().partition(kept);
        i.events = rest;
        kept
    })
}

/// Sets the menu's keys aside while a menu is open going into this frame. Call before anything
/// reads keys (the shortcuts, the views); once per pass, so the menu wrappers call it too,
/// for egui's menus that read Esc before drawing their contents.
pub(crate) fn capture(ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    let mut nav = memory::load(ctx);
    if nav.stash_pass == pass {
        return;
    }
    nav.stash_pass = pass;
    nav.stash = if memory::open(&nav, pass) { take_kept(ctx) } else { Vec::new() };
    memory::store(ctx, nav);
}

/// Gives the root menu's contents the keys set aside for them.
pub(super) fn restore(ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    let mut nav = memory::load(ctx);
    if nav.stash_pass != pass || nav.stash.is_empty() {
        return;
    }
    let events = std::mem::take(&mut nav.stash);
    memory::store(ctx, nav);
    ctx.input_mut(|i| i.events.extend(events));
}

/// Sets aside again what the menu left, after its contents.
pub(super) fn keep(ctx: &egui::Context) {
    let left = take_kept(ctx);
    let mut nav = memory::load(ctx);
    nav.stash_pass = ctx.cumulative_pass_nr();
    nav.stash = left;
    memory::store(ctx, nav);
}

/// Takes a press of `key` alone, from the input or from what the open menu set aside.
pub(crate) fn take_key(ctx: &egui::Context, key: Key) -> bool {
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
        return true;
    }
    let pass = ctx.cumulative_pass_nr();
    let mut nav = memory::load(ctx);
    if nav.stash_pass != pass {
        return false;
    }
    let at =
        nav.stash.iter().position(|e| matches!(e, Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && modifiers.is_none()));
    let Some(at) = at else { return false };
    nav.stash.remove(at);
    memory::store(ctx, nav);
    true
}
