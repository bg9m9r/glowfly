//! Ember and Pine, the warm-accent darks. Ember: charcoal with a faint red lean (OKLCH hue 10,
//! chroma about 0.014, so it never reads brown) under a flame-orange accent in its own band (61
//! to 75 deg, just above the coral's), a clean deep-red selected row, and the flame branch
//! families (`branch_palette_more.rs`). Pine: slate charcoal with a hint of spruce (hue 212,
//! chroma about 0.017) under the lantern gold, a petrol-blue selected row, and Dusk's gold
//! families. The values and the rules they meet are in docs/COLOUR.md.

use super::branch_palette_gold::{BRANCH_CB_DUSK, BRANCH_DUSK};
use super::branch_palette_more::{BRANCH_CB_EMBER, BRANCH_EMBER};
use super::builtin_extra::REPO_DARK;
use super::color::Palette;
use super::families::{Attention, Status};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Red-leaning charcoal neutrals under a flame-orange accent.
pub const EMBER: Palette = Palette {
    dark: true,
    bg: hex(0x211819),
    panel: hex(0x281f20),
    toolbar: hex(0x181112),
    raised: hex(0x372e2f),
    input: hex(0x1b1415),
    border: hex(0x3e3435),
    input_border: hex(0x938587),
    dialog_border: hex(0x938587),
    hover: hex(0x3a2e30),
    selected: hex(0x7c1d1e),
    text: hex(0xebe6e6),
    strong: hex(0xffffff),
    weak: hex(0xc0b6b7),
    faint: hex(0xa5999b),
    accent: hex(0xff9b2a),
    primary: hex(0xff9b2a),
    on_primary: hex(0x1b1113),
    primary_off: hex(0x2e2526),
    primary_border: hex(0xd97f01),
    danger: hex(0x8a2631),
    danger_off: hex(0x322325),
    tag_bg: hex(0x4f2a3c),
    tag_fg: hex(0xf7c8dd),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xe3b842), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xe9c447), banner: hex(0x312829), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
    diff_add: hex(0x1c3723),
    diff_del: hex(0x472024),
    diff_add_sel: hex(0x2a5c3a),
    diff_del_sel: hex(0x6a2c31),
    diff_add_word: hex(0x2c623d),
    diff_del_word: hex(0x76303a),
    hunk_bg: hex(0x2c243c),
    hunk_fg: hex(0xc2afe7),
    syn_comment: hex(0xb09799),
    syn_string: hex(0xa8cc8c),
    syn_number: hex(0xe0a46e),
    syn_constant: hex(0x6cc8d8),
    syn_keyword: hex(0xc48ee6),
    syn_type: hex(0xe6c07a),
    syn_function: hex(0x6faef2),
    syn_attr: hex(0x8ac6d0),
    repository: REPO_DARK,
    branch: BRANCH_EMBER,
    colour_blind: BRANCH_CB_EMBER,
    ansi: super::ansi::EMBER,
};

/// Spruce-slate charcoal neutrals under the lantern gold.
pub const PINE: Palette = Palette {
    dark: true,
    bg: hex(0x111d1f),
    panel: hex(0x182426),
    toolbar: hex(0x0b1517),
    raised: hex(0x273336),
    input: hex(0x0d181a),
    border: hex(0x2c393c),
    input_border: hex(0x7b8c90),
    dialog_border: hex(0x7b8c90),
    hover: hex(0x263538),
    selected: hex(0x0e4b5c),
    text: hex(0xe3e8ea),
    strong: hex(0xffffff),
    weak: hex(0xafbcbf),
    faint: hex(0x91a0a3),
    accent: hex(0xf6c159),
    primary: hex(0xf6c159),
    on_primary: hex(0x091619),
    primary_off: hex(0x1f2a2d),
    primary_border: hex(0xd5a13c),
    danger: hex(0x852531),
    danger_off: hex(0x371e23),
    tag_bg: hex(0x4a283f),
    tag_fg: hex(0xf4c9e4),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xf69d3d), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xfba541), banner: hex(0x212d30), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
    diff_add: hex(0x1c3723),
    diff_del: hex(0x472024),
    diff_add_sel: hex(0x2a5c3a),
    diff_del_sel: hex(0x6a2c31),
    diff_add_word: hex(0x2c623d),
    diff_del_word: hex(0x76303a),
    hunk_bg: hex(0x172a3e),
    hunk_fg: hex(0x94bfed),
    syn_comment: hex(0x91a2a3),
    syn_string: hex(0xa8cc8c),
    syn_number: hex(0xe0a46e),
    syn_constant: hex(0x6cc8d8),
    syn_keyword: hex(0xc48ee6),
    syn_type: hex(0xe6c07a),
    syn_function: hex(0x6faef2),
    syn_attr: hex(0x8ac6d0),
    repository: REPO_DARK,
    branch: BRANCH_DUSK,
    colour_blind: BRANCH_CB_DUSK,
    ansi: super::ansi::PINE,
};
