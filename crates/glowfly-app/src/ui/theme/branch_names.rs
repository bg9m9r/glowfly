//! The branch hues' names in the language, for a branch menu's Color submenu. Each branch family
//! names its slots (`Branch::names`), so Dusk and Dawn's Jade and Orange read as themselves.

use super::families::{Branch, LANE_COUNT};
use glowfly_core::i18n::Lang;

/// A branch hue's name. Red, Lime, Plum and Orange appear in more than one family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Rose,
    Lime,
    Violet,
    Orange,
    Cyan,
    Magenta,
    Olive,
    Azure,
    Red,
    Teal,
    Orchid,
    Gold,
    Jade,
    Sky,
    Indigo,
    Aqua,
    Yellow,
    Blue,
    Plum,
    Purple,
    Periwinkle,
}

impl Hue {
    /// The name in the language.
    pub fn name(self, t: Lang) -> &'static str {
        match self {
            Hue::Rose => t.tr("branch.hue.rose"),
            Hue::Lime => t.tr("branch.hue.lime"),
            Hue::Violet => t.tr("branch.hue.violet"),
            Hue::Orange => t.tr("branch.hue.orange"),
            Hue::Cyan => t.tr("branch.hue.cyan"),
            Hue::Magenta => t.tr("branch.hue.magenta"),
            Hue::Olive => t.tr("branch.hue.olive"),
            Hue::Azure => t.tr("branch.hue.azure"),
            Hue::Red => t.tr("branch.hue.red"),
            Hue::Teal => t.tr("branch.hue.teal"),
            Hue::Orchid => t.tr("branch.hue.orchid"),
            Hue::Gold => t.tr("branch.hue.gold"),
            Hue::Jade => t.tr("branch.hue.jade"),
            Hue::Sky => t.tr("branch.hue.sky"),
            Hue::Indigo => t.tr("branch.hue.indigo"),
            Hue::Aqua => t.tr("branch.hue.aqua"),
            Hue::Yellow => t.tr("branch.hue.yellow"),
            Hue::Blue => t.tr("branch.hue.blue"),
            Hue::Plum => t.tr("branch.hue.plum"),
            Hue::Purple => t.tr("branch.hue.purple"),
            Hue::Periwinkle => t.tr("branch.hue.periwinkle"),
        }
    }
}

/// The name of hue `slot` (taken modulo the family's hue count) in the branch family `b`.
pub fn hue(t: Lang, b: &Branch, slot: u8) -> &'static str {
    b.names[usize::from(slot % b.preset.slots()) % LANE_COUNT].name(t)
}
