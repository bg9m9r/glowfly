//! A menu's columns, shared by all its rows so they line up (the menu review's spec §3), left to
//! right inside a row's text inset:
//!
//! - the state column (`STATE_W`): a painted ✓ or a radio dot, kept on every row once any row
//!   has a mark;
//! - the icon column (`ICON_W`): a branch dot or a repository swatch, kept the same way;
//! - the label, at least `LABEL_GAP` from what follows;
//! - the reason slot (`INFO_SLOT`): a blocked row's info mark, at one x on every row;
//! - the shortcut column, right-aligned, as wide as the widest key (or "Set up…");
//! - the chevron slot (`CHEVRON_W`), kept on every row once the menu has a submenu.
//!
//! Each row says what it needs (`note`), and the menu remembers the union for its next pass
//! (`begin` and `end`, run by `menu::styled` around every menu's contents). When the union
//! changes (the menu's first pass, or new rows), the pass is discarded and drawn again, so a
//! menu never shows misaligned for a frame. A row outside a menu lines up with itself only.

use eframe::egui::{Id, Ui};

/// The state column's width, the icon column's, and the gap after each.
pub const STATE_W: f32 = 16.0;
pub const ICON_W: f32 = 16.0;
pub const LEAD_GAP: f32 = 8.0;
/// The least space between a label and the reason, shortcut or chevron after it.
pub const LABEL_GAP: f32 = 24.0;
/// The reason slot's width, the chevron slot's, and the gap between two trailing slots.
pub const INFO_SLOT: f32 = 16.0;
pub const CHEVRON_W: f32 = 12.0;
pub const SLOT_GAP: f32 = 8.0;

/// What a menu's rows need room for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Columns {
    /// A check or radio mark.
    pub state: bool,
    /// A dot or swatch.
    pub icon: bool,
    /// A blocked row's info mark.
    pub info: bool,
    /// The widest shortcut (or setup suffix).
    pub key_w: f32,
    /// A submenu's chevron.
    pub chevron: bool,
}

impl Columns {
    /// Room for what either needs.
    pub fn union(self, o: Self) -> Self {
        Self {
            state: self.state || o.state,
            icon: self.icon || o.icon,
            info: self.info || o.info,
            key_w: self.key_w.max(o.key_w),
            chevron: self.chevron || o.chevron,
        }
    }

    /// The state column's centre, from the row's left edge (`pad` is the text inset).
    pub fn state_x(&self, pad: f32) -> f32 {
        pad + STATE_W / 2.0
    }

    /// The icon column's left edge, from the row's left edge.
    pub fn icon_left(&self, pad: f32) -> f32 {
        pad + if self.state { STATE_W + LEAD_GAP } else { 0.0 }
    }

    /// Where the labels start, from the row's left edge: also where a menu's headers and notes go.
    pub fn label_x(&self, pad: f32) -> f32 {
        self.icon_left(pad) + if self.icon { ICON_W + LEAD_GAP } else { 0.0 }
    }

    /// The shortcut column's right edge, in from the row's right edge.
    pub fn key_right(&self, pad: f32) -> f32 {
        pad + if self.chevron { CHEVRON_W + SLOT_GAP } else { 0.0 }
    }

    /// The reason slot's right edge, in from the row's right edge.
    pub fn info_right(&self, pad: f32) -> f32 {
        self.key_right(pad) + if self.key_w > 0.0 { self.key_w + SLOT_GAP } else { 0.0 }
    }

    /// The room right of the label: the gap and the trailing slots, or nothing without any.
    pub fn trailing(&self) -> f32 {
        let slots = [(self.info, INFO_SLOT), (self.key_w > 0.0, self.key_w), (self.chevron, CHEVRON_W)];
        let (n, w) = slots.iter().filter(|(on, _)| *on).fold((0, 0.0), |(n, w), (_, s)| (n + 1, w + s));
        if n == 0 { 0.0 } else { LABEL_GAP + w + SLOT_GAP * (n - 1) as f32 }
    }

    /// A row's least width for a label `label_w` wide.
    pub fn row_w(&self, pad: f32, label_w: f32) -> f32 {
        self.label_x(pad) + label_w + self.trailing() + pad
    }
}

/// What a menu used last pass and what its rows asked for so far this pass.
#[derive(Clone, Copy, Debug, Default)]
struct Learned {
    used: Columns,
    seen: Columns,
    pass: u64,
}

fn key(ui: &Ui) -> Id {
    Id::new((ui.layer_id(), "menu-columns"))
}

fn load(ui: &Ui) -> Learned {
    ui.data(|d| d.get_temp::<Learned>(key(ui))).unwrap_or_default()
}

fn store(ui: &Ui, l: Learned) {
    ui.data_mut(|d| d.insert_temp(key(ui), l));
}

/// Starts a menu's pass: its rows' needs are counted afresh.
pub(super) fn begin(ui: &Ui) {
    let pass = ui.ctx().cumulative_pass_nr();
    store(ui, Learned { seen: Columns::default(), pass, ..load(ui) });
}

/// Ends a menu's pass: when its rows needed other columns than it laid out, keeps theirs and
/// draws the pass again.
pub(super) fn end(ui: &Ui) {
    let l = load(ui);
    if l.seen != l.used {
        store(ui, Learned { used: l.seen, ..l });
        ui.ctx().request_discard("menu columns");
    }
}

/// The columns `ui`'s menu lays its rows out in.
pub fn used(ui: &Ui) -> Columns {
    load(ui).used
}

/// Records that a row of `ui`'s menu needs `need`, and returns the columns to lay it out in.
pub fn note(ui: &Ui, need: Columns) -> Columns {
    let pass = ui.ctx().cumulative_pass_nr();
    let mut l = load(ui);
    if l.pass != pass {
        l = Learned { seen: Columns::default(), pass, ..l };
    }
    l.seen = l.seen.union(need);
    store(ui, l);
    l.used.union(need)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_move_right_for_marks_and_icons() {
        let none = Columns::default();
        assert_eq!(none.label_x(12.0), 12.0);
        let marked = Columns { state: true, ..none };
        assert_eq!(marked.label_x(12.0), 12.0 + 16.0 + 8.0, "the spec's 40 from the frame with its 4 px pad");
        let both = Columns { icon: true, ..marked };
        assert_eq!(both.icon_left(12.0), 36.0);
        assert_eq!(both.label_x(12.0), 60.0);
    }

    #[test]
    fn trailing_slots_sit_at_fixed_offsets() {
        let c = Columns { info: true, key_w: 50.0, chevron: true, ..Columns::default() };
        assert_eq!(c.key_right(12.0), 12.0 + 12.0 + 8.0);
        assert_eq!(c.info_right(12.0), 32.0 + 50.0 + 8.0);
        assert_eq!(c.trailing(), 24.0 + 16.0 + 50.0 + 12.0 + 2.0 * 8.0);
        assert_eq!(Columns::default().trailing(), 0.0);
    }
}
