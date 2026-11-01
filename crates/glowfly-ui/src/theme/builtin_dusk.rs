//! Dusk and Dawn, the pair drawn from the app icon: its dusk tile (indigo into plum) tints the
//! neutrals, and the glowfly's lantern gold is Dusk's accent, so the main branch runs in gold as
//! the icon's first lane does; Dusk's selected row is an indigo step under the gold bar. On
//! Dawn's paper a gold line dark enough to read is brown, so Dawn's one accent is an iris violet
//! (OKLCH hue 283, the dawn sky over the lavender paper), on its buttons and selected row too.
//! Tags take the plum of the tile's lower edge. `modified` and `warn` sit on the orange side of the gold (OKLCH hue about
//! 64), out of the gold accent's band. The values and the rules they meet are in
//! docs/COLOUR.md.

use super::branch_palette_gold::{BRANCH_CB_DAWN, BRANCH_CB_DUSK, BRANCH_DAWN, BRANCH_DUSK};
use super::builtin_extra::{REPO_DARK, REPO_LIGHT};
use super::color::Palette;
use super::families::{Attention, Status};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Indigo neutrals under a lantern-gold accent.
pub const DUSK: Palette = Palette {
    dark: true,
    bg: hex(0x1b1829),
    panel: hex(0x221f31),
    toolbar: hex(0x131220),
    raised: hex(0x312d41),
    input: hex(0x161423),
    border: hex(0x373347),
    input_border: hex(0x89839e),
    dialog_border: hex(0x89839e),
    hover: hex(0x332d44),
    selected: hex(0x362f58),
    text: hex(0xe6e5ef),
    strong: hex(0xffffff),
    weak: hex(0xb6b3c8),
    faint: hex(0x9b96ae),
    accent: hex(0xf6c159),
    primary: hex(0xf6c159),
    on_primary: hex(0x151229),
    primary_off: hex(0x292536),
    primary_border: hex(0xd5a13c),
    danger: hex(0x852531),
    danger_off: hex(0x371e22),
    tag_bg: hex(0x4a283f),
    tag_fg: hex(0xf4c9e4),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xf69d3d), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xfba541), banner: hex(0x2b273b), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
    diff_add: hex(0x1c3723),
    diff_del: hex(0x472024),
    diff_add_sel: hex(0x2a5c3a),
    diff_del_sel: hex(0x6a2c31),
    diff_add_word: hex(0x2c623d),
    diff_del_word: hex(0x76303a),
    hunk_bg: hex(0x252541),
    hunk_fg: hex(0xacb1f0),
    syn_comment: hex(0xa29eb6),
    syn_string: hex(0xa8cc8c),
    syn_number: hex(0xe4a266),
    syn_constant: hex(0x6cc8d8),
    syn_keyword: hex(0xc48ee6),
    syn_type: hex(0xe6c07a),
    syn_function: hex(0x6faef2),
    syn_attr: hex(0x8ac6d0),
    repository: REPO_DARK,
    branch: BRANCH_DUSK,
    colour_blind: BRANCH_CB_DUSK,
    ansi: super::ansi::DUSK,
};

/// Faintly lavender paper and indigo ink under an iris accent.
pub const DAWN: Palette = Palette {
    dark: false,
    bg: hex(0xf7f6fc),
    panel: hex(0xfbfafe),
    toolbar: hex(0xe7e5ef),
    raised: hex(0xedebf4),
    input: hex(0xffffff),
    border: hex(0xd8d6e1),
    input_border: hex(0x747188),
    dialog_border: hex(0x747188),
    hover: hex(0xece9f5),
    selected: hex(0xe6e1fa),
    text: hex(0x211f38),
    strong: hex(0x100d26),
    weak: hex(0x54526d),
    faint: hex(0x6b6882),
    accent: hex(0x5a47cf),
    primary: hex(0x5a47cf),
    on_primary: hex(0xffffff),
    primary_off: hex(0xe6e5ee),
    primary_border: hex(0x4535b5),
    danger: hex(0xc0332a),
    danger_off: hex(0xfedadc),
    tag_bg: hex(0xf5d8ea),
    tag_fg: hex(0x763463),
    status: Status { added: hex(0x357518), removed: hex(0xb63132), modified: hex(0x985700), info: hex(0x1f68bc), progress: hex(0x6d47b8) },
    attention: Attention { warn: hex(0x985700), banner: hex(0xfee3c5), needs_you: hex(0xbf4306), on_needs_you: hex(0xffffff) },
    diff_add: hex(0xe4f6e7),
    diff_del: hex(0xfbe6e6),
    diff_add_sel: hex(0xb8e6c1),
    diff_del_sel: hex(0xf5c2c2),
    diff_add_word: hex(0xa8dfb4),
    diff_del_word: hex(0xf2b4b4),
    hunk_bg: hex(0xe9e9fc),
    hunk_fg: hex(0x42428c),
    syn_comment: hex(0x5f5d78),
    syn_string: hex(0x1a6f30),
    syn_number: hex(0xc92d00),
    syn_constant: hex(0x006f7e),
    syn_keyword: hex(0x8a2bb0),
    syn_type: hex(0x995a00),
    syn_function: hex(0x1f5fbf),
    syn_attr: hex(0x006b75),
    repository: REPO_LIGHT,
    branch: BRANCH_DAWN,
    colour_blind: BRANCH_CB_DAWN,
    ansi: super::ansi::DAWN,
};
