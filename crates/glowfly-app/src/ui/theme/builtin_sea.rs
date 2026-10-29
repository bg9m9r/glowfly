//! Midnight and Mist, the sea-glass pair: a deep blue-black night and its light twin of cool
//! fog paper. The neutrals sit on one blue hue (OKLCH 235 to 255); the accent is a sea-glass
//! green-aqua at the cool edge of the green band, so the Garden branch families fit as they are.
//! The values and the rules they meet are in docs/COLOUR.md.

use super::branch_palette::{BRANCH_CB_DARK, BRANCH_CB_LIGHT, BRANCH_DARK, BRANCH_LIGHT};
use super::builtin_extra::{REPO_DARK, REPO_LIGHT};
use super::color::Palette;
use super::families::{Attention, Status};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Deep blue-black night: blue neutrals under a sea-glass accent.
pub const MIDNIGHT: Palette = Palette {
    dark: true,
    bg: hex(0x0d1827),
    panel: hex(0x14202f),
    toolbar: hex(0x08111d),
    raised: hex(0x222f40),
    input: hex(0x0a1421),
    border: hex(0x283545),
    input_border: hex(0x798ba2),
    dialog_border: hex(0x798ba2),
    hover: hex(0x213044),
    selected: hex(0x174a39),
    text: hex(0xe2e8f0),
    strong: hex(0xffffff),
    weak: hex(0xadbacc),
    faint: hex(0x8f9eb3),
    accent: hex(0x6eddb2),
    primary: hex(0x6eddb2),
    on_primary: hex(0x08121f),
    primary_off: hex(0x1b2634),
    primary_border: hex(0x4eb891),
    danger: hex(0x852531),
    danger_off: hex(0x371e26),
    tag_bg: hex(0x3d2d4e),
    tag_fg: hex(0xe0cef8),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xedb345), info: hex(0x60aaf3), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xe8b339), banner: hex(0x1c2939), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
    diff_add: hex(0x1c3723),
    diff_del: hex(0x472024),
    diff_add_sel: hex(0x2a5c3a),
    diff_del_sel: hex(0x6a2c31),
    diff_add_word: hex(0x2c623d),
    diff_del_word: hex(0x76303a),
    hunk_bg: hex(0x19263f),
    hunk_fg: hex(0x9bbbf5),
    syn_comment: hex(0x92a0b1),
    syn_string: hex(0xa8cc8c),
    syn_number: hex(0xe4a266),
    syn_constant: hex(0x6cc8d8),
    syn_keyword: hex(0xc48ee6),
    syn_type: hex(0xe6c07a),
    syn_function: hex(0x6faef2),
    syn_attr: hex(0x8ac6d0),
    repository: REPO_DARK,
    branch: BRANCH_DARK,
    colour_blind: BRANCH_CB_DARK,
    ansi: super::ansi::MIDNIGHT,
};

/// Midnight's light twin: cool fog paper, blue-grey ink and deep sea glass.
pub const MIST: Palette = Palette {
    dark: false,
    bg: hex(0xf2f8fc),
    panel: hex(0xf8fcfe),
    toolbar: hex(0xdee9f0),
    raised: hex(0xe5eff5),
    input: hex(0xffffff),
    border: hex(0xcfdae1),
    input_border: hex(0x687683),
    dialog_border: hex(0x687683),
    hover: hex(0xe2eef5),
    selected: hex(0xc7eddd),
    text: hex(0x142533),
    strong: hex(0x041320),
    weak: hex(0x475867),
    faint: hex(0x5e6f7d),
    accent: hex(0x007258),
    primary: hex(0x007258),
    on_primary: hex(0xffffff),
    primary_off: hex(0xdfe8ed),
    primary_border: hex(0x005641),
    danger: hex(0xc0332a),
    danger_off: hex(0xfbdcdc),
    tag_bg: hex(0xe7dbf7),
    tag_fg: hex(0x5e3d80),
    status: Status { added: hex(0x357518), removed: hex(0xb63132), modified: hex(0x975600), info: hex(0x1f68bc), progress: hex(0x6d47b8) },
    attention: Attention { warn: hex(0x975600), banner: hex(0xfbecc8), needs_you: hex(0xbf4306), on_needs_you: hex(0xffffff) },
    diff_add: hex(0xe4f6e7),
    diff_del: hex(0xfbe6e6),
    diff_add_sel: hex(0xb8e6c1),
    diff_del_sel: hex(0xf5c2c2),
    diff_add_word: hex(0xa8dfb4),
    diff_del_word: hex(0xf2b4b4),
    hunk_bg: hex(0xe1edfa),
    hunk_fg: hex(0x1d4d87),
    syn_comment: hex(0x556574),
    syn_string: hex(0x1a6f30),
    syn_number: hex(0xc92d00),
    syn_constant: hex(0x006f7e),
    syn_keyword: hex(0x8a2bb0),
    syn_type: hex(0x995a00),
    syn_function: hex(0x1f5fbf),
    syn_attr: hex(0x006b75),
    repository: REPO_LIGHT,
    branch: BRANCH_LIGHT,
    colour_blind: BRANCH_CB_LIGHT,
    ansi: super::ansi::MIST,
};
