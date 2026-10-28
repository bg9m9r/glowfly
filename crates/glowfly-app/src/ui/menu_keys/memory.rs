//! What the menu keyboard keeps between frames, in egui's temp data: the menu levels drawn
//! this pass and the last (root first, a submenu after its parent), how deep the contents being
//! drawn now are, whether the last input came from the keyboard, the keys set aside for an open
//! menu (`stash.rs`), and the requests between a submenu's row and its level.

use super::Kind;
use eframe::egui::{self, Event, Id, LayerId, Rect};

/// One open menu (or submenu): its layer, what it is, the item that last had the focus in it and
/// the widget the focus goes back to when it closes.
#[derive(Clone, Debug)]
pub(super) struct Level {
    pub layer: LayerId,
    pub kind: Kind,
    pub last_focus: Option<Id>,
    pub invoker: Option<Id>,
    /// Its first item takes the focus once it can (it opened from the keyboard, or Down with
    /// nothing focused), or its last (Up).
    pub first: bool,
    pub last: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Nav {
    /// The pass `levels` belong to.
    pub pass: u64,
    pub levels: Vec<Level>,
    /// The levels of the pass before `pass` (empty when no menu was drawn then).
    pub prev: Vec<Level>,
    /// How many menus' contents are being drawn now, one inside the other.
    pub depth: usize,
    /// Whether the latest key or pointer input was a key press.
    pub keyboard: bool,
    /// The pass `keyboard` was last brought up to date in.
    pub observed: u64,
    /// Layers whose first item takes the focus once it can.
    pub pending: Vec<(u64, LayerId)>,
    /// Submenus asked to close this pass, and whether by Esc (else Left).
    pub close: Vec<(LayerId, bool)>,
    /// The keys set aside for the open menu, and the pass they belong to.
    pub stash: Vec<Event>,
    pub stash_pass: u64,
    /// The pass in which Esc closed the root menu: no menu is open after it.
    pub closed_pass: u64,
    /// Where keyboard-opened submenus were drawn, and when.
    pub keyed: Vec<(u64, Rect)>,
    /// Layers whose submenu rows leave Enter, Space and Right to the code drawing the menu.
    pub caller_opens: Vec<(u64, LayerId)>,
}

const NAV: &str = "menu-keys";

pub(super) fn load(ctx: &egui::Context) -> Nav {
    let mut nav: Nav = ctx.data(|d| d.get_temp(Id::new(NAV)).unwrap_or_default());
    roll(ctx, &mut nav);
    nav
}

pub(super) fn store(ctx: &egui::Context, nav: Nav) {
    ctx.data_mut(|d| d.insert_temp(Id::new(NAV), nav));
}

/// Brings `nav` to this pass: last pass's levels become `prev`, and `keyboard` follows this
/// pass's input.
fn roll(ctx: &egui::Context, nav: &mut Nav) {
    let pass = ctx.cumulative_pass_nr();
    if nav.pass != pass {
        let levels = std::mem::take(&mut nav.levels);
        nav.prev = if nav.pass + 1 == pass { levels } else { Vec::new() };
        nav.pass = pass;
        nav.depth = 0;
        nav.close.clear();
        nav.keyed.retain(|&(p, _)| p + 1 >= pass);
        nav.caller_opens.retain(|&(p, _)| p == pass);
        nav.pending.retain(|&(p, _)| p + 1 >= pass);
    }
    if nav.observed != pass {
        nav.observed = pass;
        // The raw events: the frame's input before anything took keys out of it.
        let last = ctx.input(|i| {
            i.raw.events.iter().rev().find_map(|e| match e {
                Event::Key { pressed: true, .. } => Some(true),
                Event::PointerButton { pressed: true, .. } | Event::PointerMoved(_) => Some(false),
                _ => None,
            })
        });
        nav.keyboard = last.unwrap_or(nav.keyboard);
    }
}

/// Whether a menu is open going into this pass: one was drawn last pass and Esc didn't close it.
pub(super) fn open(nav: &Nav, pass: u64) -> bool {
    let last = if nav.pass == pass {
        &nav.prev
    } else if nav.pass + 1 == pass {
        &nav.levels
    } else {
        return false;
    };
    !last.is_empty() && nav.closed_pass + 1 != pass
}

/// Starts drawing a menu's contents; whether it is a root (no menu's contents around it).
pub(super) fn enter(ctx: &egui::Context) -> bool {
    let mut nav = load(ctx);
    let root = nav.depth == 0;
    nav.depth += 1;
    store(ctx, nav);
    root
}

/// Ends drawing a menu's contents; whether it was a root.
pub(super) fn leave(ctx: &egui::Context) -> bool {
    let mut nav = load(ctx);
    nav.depth = nav.depth.saturating_sub(1);
    let root = nav.depth == 0;
    store(ctx, nav);
    root
}

/// The open level drawn on `layer`, this pass or last.
pub(super) fn level_of(nav: &Nav, layer: LayerId) -> Option<&Level> {
    nav.levels.iter().find(|l| l.layer == layer).or_else(|| nav.prev.iter().find(|l| l.layer == layer))
}

/// The root level: the first drawn this pass, else last pass's.
pub(super) fn root(nav: &Nav) -> Option<&Level> {
    nav.levels.first().or_else(|| nav.prev.first())
}

/// The widget the opener of the menu being opened now named (`super::opener`), for this pass.
const OPENER: &str = "menu-keys-opener";

pub(super) fn set_opener(ctx: &egui::Context, id: Option<Id>) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(Id::new(OPENER), (pass, id)));
}

pub(super) fn opener(ctx: &egui::Context) -> Option<Id> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, Option<Id>)>(Id::new(OPENER))).filter(|&(p, _)| p == pass).and_then(|(_, id)| id)
}
