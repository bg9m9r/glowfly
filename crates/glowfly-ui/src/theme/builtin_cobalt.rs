//! Graphite and Porcelain, the cobalt pair and the defaults: near-neutral graphite greys (chroma
//! under 0.01, a hair cool) and neutral white paper with grey-black ink, both under a cobalt
//! accent in its own band (243 to 259 deg) with the cobalt branch families
//! (`branch_palette_more.rs`). `info` moves to hue about 224 to make room. Porcelain keeps
//! Linen's (the theme it replaced) status, diff, syntax and branch colours. The values and the
//! rules they meet are in docs/COLOUR.md.

use super::branch_palette_more::{BRANCH_CB_GRAPHITE, BRANCH_CB_LINEN, BRANCH_GRAPHITE, BRANCH_LINEN};
use super::builtin_extra::{REPO_DARK, REPO_LIGHT};
use super::color::Palette;
use super::families::{Attention, Status};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Near-neutral graphite greys with a cobalt accent.
pub const GRAPHITE: Palette = Palette {
    dark: true,
    bg: hex(0x191a1d),
    panel: hex(0x202225),
    toolbar: hex(0x121315),
    raised: hex(0x2f3134),
    input: hex(0x141618),
    border: hex(0x35373a),
    input_border: hex(0x868990),
    dialog_border: hex(0x868990),
    hover: hex(0x303236),
    selected: hex(0x1f3d62),
    text: hex(0xe6e7e9),
    strong: hex(0xffffff),
    weak: hex(0xb7b9bc),
    faint: hex(0x9a9da2),
    accent: hex(0x7ebbff),
    primary: hex(0x7ebbff),
    on_primary: hex(0x0a1018),
    primary_off: hex(0x26282b),
    primary_border: hex(0x5695e0),
    danger: hex(0x852531),
    danger_off: hex(0x372023),
    tag_bg: hex(0x3e314d),
    tag_fg: hex(0xe0cff5),
    status: Status { added: hex(0x7bc85f), removed: hex(0xea6a64), modified: hex(0xedb345), info: hex(0x4fc0e7), progress: hex(0xa689f1) },
    attention: Attention { warn: hex(0xe8b339), banner: hex(0x292b2e), needs_you: hex(0xfb794a), on_needs_you: hex(0x22110b) },
    diff_add: hex(0x1c3723),
    diff_del: hex(0x472024),
    diff_add_sel: hex(0x2a5c3a),
    diff_del_sel: hex(0x6a2c31),
    diff_add_word: hex(0x2c623d),
    diff_del_word: hex(0x76303a),
    hunk_bg: hex(0x232838),
    hunk_fg: hex(0xa9b9e8),
    code_sel: hex(0x1d4f82),
    code_sel_dim: hex(0x474e56),
    syn_comment: hex(0x9b9ea5),
    syn_string: hex(0xa8cc8c),
    syn_number: hex(0xe4a266),
    syn_constant: hex(0x6cc8d8),
    syn_keyword: hex(0xc48ee6),
    syn_type: hex(0xe6c07a),
    syn_function: hex(0x6faef2),
    syn_attr: hex(0x8ac6d0),
    repository: REPO_DARK,
    branch: BRANCH_GRAPHITE,
    colour_blind: BRANCH_CB_GRAPHITE,
    ansi: super::ansi::GRAPHITE,
};

/// Neutral white paper, grey-black ink and cobalt: Graphite's light twin.
pub const PORCELAIN: Palette = Palette {
    dark: false,
    bg: hex(0xf6f7f9),
    panel: hex(0xfbfbfc),
    toolbar: hex(0xe9ebee),
    raised: hex(0xeef0f3),
    input: hex(0xffffff),
    border: hex(0xdadde2),
    input_border: hex(0x737882),
    dialog_border: hex(0x737882),
    hover: hex(0xeceef2),
    selected: hex(0xdbe8fc),
    text: hex(0x1e2228),
    strong: hex(0x0c0f13),
    weak: hex(0x525862),
    faint: hex(0x676d77),
    accent: hex(0x1a5bb5),
    primary: hex(0x1a5bb5),
    on_primary: hex(0xffffff),
    primary_off: hex(0xe8eaed),
    primary_border: hex(0x134894),
    danger: hex(0xc0332a),
    danger_off: hex(0xfbdcdc),
    tag_bg: hex(0xf8dae2),
    tag_fg: hex(0x813450),
    status: Status { added: hex(0x357518), removed: hex(0xb63132), modified: hex(0x985700), info: hex(0x006e91), progress: hex(0x6d47b8) },
    attention: Attention { warn: hex(0x985700), banner: hex(0xfbecc8), needs_you: hex(0xbf4306), on_needs_you: hex(0xffffff) },
    diff_add: hex(0xe4f6e7),
    diff_del: hex(0xfbe6e6),
    diff_add_sel: hex(0xb8e6c1),
    diff_del_sel: hex(0xf5c2c2),
    diff_add_word: hex(0xa8dfb4),
    diff_del_word: hex(0xf2b4b4),
    hunk_bg: hex(0xe6edfb),
    hunk_fg: hex(0x2d4d8b),
    code_sel: hex(0xb9d5fe),
    code_sel_dim: hex(0xd8dfe9),
    syn_comment: hex(0x686e78),
    syn_string: hex(0x1a6f30),
    syn_number: hex(0xc92d00),
    syn_constant: hex(0x006f7e),
    syn_keyword: hex(0x8a2bb0),
    syn_type: hex(0x995a00),
    syn_function: hex(0x1f5fbf),
    syn_attr: hex(0x006b75),
    repository: REPO_LIGHT,
    branch: BRANCH_LINEN,
    colour_blind: BRANCH_CB_LINEN,
    ansi: super::ansi::PORCELAIN,
};
