//! Ember and Pine, the warm-accent darks. Ember: oxblood charcoal (OKLCH hue 22, red-leaning so
//! it never reads brown or olive) under a flame-orange accent in its own band (61 to 75 deg,
//! just above the coral's), a deep ember-red selected row, and the flame branch families
//! (`branch_palette_more.rs`). Pine: blue-green charcoal (hue 194, chroma near Dusk's, so the
//! green is a hint, not a wash) under the lantern gold, with Dusk's gold families. The values
//! and the rules they meet are in docs/COLOUR.md.

use super::branch_palette_gold::{BRANCH_CB_DUSK, BRANCH_DUSK};
use super::branch_palette_more::{BRANCH_CB_EMBER, BRANCH_EMBER};
use super::builtin_extra::REPO_DARK;
use super::color::Palette;
use super::families::{Attention, Status};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Oxblood charcoal neutrals under a flame-orange accent.
pub const EMBER: Palette = Palette {
    dark: true,
    bg: hex(0x261517),
    panel: hex(0x2e1c1e),
    toolbar: hex(0x1c0e10),
    raised: hex(0x3e2b2d),
    input: hex(0x201112),
    border: hex(0x443133),
    input_border: hex(0x9d8183),
    dialog_border: hex(0x9d8183),
    hover: hex(0x412b2d),
    selected: hex(0x692726),
    text: hex(0xefe4e5),
    strong: hex(0xffffff),
    weak: hex(0xc8b3b5),
    faint: hex(0xae9698),
    accent: hex(0xff9b2a),
    primary: hex(0xff9b2a),
    on_primary: hex(0x210d10),
    primary_off: hex(0x342224),
    primary_border: hex(0xd97f01),
    danger: hex(0x8a2631),
    danger_off: hex(0x3a1e25),
    tag_bg: hex(0x4f2a3c),
    tag_fg: hex(0xf7c8dd),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xe3b842), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xe9c447), banner: hex(0x3b2f0a), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
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

/// Deep blue-green charcoal neutrals under the lantern gold.
pub const PINE: Palette = Palette {
    dark: true,
    bg: hex(0x0a1e1f),
    panel: hex(0x112527),
    toolbar: hex(0x061617),
    raised: hex(0x203536),
    input: hex(0x07191a),
    border: hex(0x263b3c),
    input_border: hex(0x738f91),
    dialog_border: hex(0x738f91),
    hover: hex(0x1d3738),
    selected: hex(0x5b3d0a),
    text: hex(0xe0e9ea),
    strong: hex(0xffffff),
    weak: hex(0xa9bebe),
    faint: hex(0x8aa2a4),
    accent: hex(0xf6c159),
    primary: hex(0xf6c159),
    on_primary: hex(0x021819),
    primary_off: hex(0x182c2d),
    primary_border: hex(0xd5a13c),
    danger: hex(0x852531),
    danger_off: hex(0x35201f),
    tag_bg: hex(0x4a283f),
    tag_fg: hex(0xf4c9e4),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xf69d3d), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xfba541), banner: hex(0x46290b), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
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
