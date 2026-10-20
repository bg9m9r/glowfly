//! The branch hues' names in the language, for a branch menu's Color submenu. Red and Lime are
//! in both presets and share their names.

use trellis_core::i18n::Lang;
use trellis_core::settings::BranchPalette;

/// The name of hue `slot` (taken modulo the preset's hue count) in `preset`.
pub fn hue(t: Lang, preset: BranchPalette, slot: u8) -> &'static str {
    let slot = slot % preset.slots();
    match preset {
        BranchPalette::Standard => match slot {
            0 => t.tr("branch.hue.rose"),
            1 => t.tr("branch.hue.lime"),
            2 => t.tr("branch.hue.violet"),
            3 => t.tr("branch.hue.orange"),
            4 => t.tr("branch.hue.cyan"),
            5 => t.tr("branch.hue.magenta"),
            6 => t.tr("branch.hue.olive"),
            7 => t.tr("branch.hue.azure"),
            8 => t.tr("branch.hue.red"),
            9 => t.tr("branch.hue.teal"),
            10 => t.tr("branch.hue.orchid"),
            11 => t.tr("branch.hue.gold"),
            12 => t.tr("branch.hue.sky"),
            _ => t.tr("branch.hue.indigo"),
        },
        BranchPalette::ColourBlind => match slot {
            0 => t.tr("branch.hue.red"),
            1 => t.tr("branch.hue.aqua"),
            2 => t.tr("branch.hue.yellow"),
            3 => t.tr("branch.hue.blue"),
            4 => t.tr("branch.hue.plum"),
            5 => t.tr("branch.hue.lime"),
            6 => t.tr("branch.hue.purple"),
            _ => t.tr("branch.hue.periwinkle"),
        },
    }
}
