//! The colours of the graph's ref chips (docs/COLOUR.md, "Ref chips"): filled for a local
//! branch, outlined for a remote branch or a tag, and a halo round the checked-out branch or a
//! detached HEAD, each worked out so the chip stands apart from every row surface it can sit
//! on (`surfaces::row_surfaces`) and its text stays readable.

use super::branch_colours::{chip_in, line_in, trunk};
use super::color::Palette;
use super::contrast::{brownish, clear, from_oklch, mix, oklch, ratio, readable_on};
use super::surfaces::row_surfaces;
use eframe::egui::Color32;
use glowfly_core::branch_colour::Swatch;

/// What a ref chip in the graph stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipStyle {
    /// A detached HEAD.
    Head,
    /// The checked-out branch.
    CurrentBranch,
    /// Another local branch.
    Branch,
    /// A remote-tracking branch.
    Remote,
    Tag,
}

impl ChipStyle {
    #[cfg(test)]
    pub const ALL: [ChipStyle; 5] = [Self::Head, Self::CurrentBranch, Self::Branch, Self::Remote, Self::Tag];

    /// Whether the chip says "you are here" with a halo: the checked-out branch or a detached HEAD.
    pub fn halo(self) -> bool {
        matches!(self, Self::Head | Self::CurrentBranch)
    }
}

/// A branch's two tones: its `line` tone (lines, bands, a light chip's edge) and its `chip`
/// tone (chip letters, a dark chip's fill).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BranchTone {
    pub line: Color32,
    pub chip: Color32,
}

impl BranchTone {
    /// Swatch `s`'s tones in `p`.
    pub fn of(s: Swatch, p: &Palette) -> Self {
        Self { line: line_in(s, p), chip: chip_in(s, p) }
    }

    /// One colour for both (a ref without a swatch of its own takes its node's colour).
    pub fn plain(c: Color32) -> Self {
        Self { line: c, chip: c }
    }
}

/// How a ref chip is painted: its opaque fill (none: outlined), the 1 px edge drawn inside it
/// (if any), its text and marks, and the halo round it (the checked-out branch, a detached HEAD).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChipLook {
    pub fill: Option<Color32>,
    pub edge: Option<Color32>,
    pub text: Color32,
    pub halo: Option<Color32>,
}

/// How far a chip's fill or edge stands from every row surface, at least.
const APART: f64 = 3.0;
/// A light palette's local branch: a wash of the branch's hue at this OKLCH lightness and at
/// most this chroma.
const PALE: (f64, f64) = (0.90, 0.09);
/// How many 5% steps a colour takes at most toward white or black.
const MAX_STEPS: usize = 20;

/// The contrast a chip's text reaches: 4.5:1, or 7:1 in the high-contrast themes.
pub(crate) fn text_target(p: &Palette) -> f64 {
    if p.high_contrast { 7.0 } else { 4.5 }
}

/// A ref chip's colours for a ref of `style` on a branch of `tone` (docs/COLOUR.md, "Ref chips").
/// A local branch is filled: on a dark palette with its clear chip tone, lifted until it stands
/// 3:1 from every row surface, lettered in the background's dark; on a light one with a wash of
/// its hue, edged in its line tone pushed to 3:1 from every row surface. A remote branch and a
/// tag are outlined: no fill, an edge and text in the branch's tone (`tag_fg` for a tag),
/// pushed until they stand 3:1 and read at [`text_target`] on every row surface. The main
/// branch takes the trunk tone; a detached HEAD looks like a local branch of the trunk. The
/// checked-out branch and HEAD add a halo in the fill (dark) or edge (light) colour.
pub fn ref_chip(style: ChipStyle, tone: BranchTone, p: &Palette) -> ChipLook {
    let rows = row_surfaces(p);
    let trunk_tone = trunk(p);
    let tone = if style == ChipStyle::Head || tone.chip == trunk_tone { BranchTone::plain(trunk_tone) } else { tone };
    let on_trunk = tone.chip == trunk_tone;
    let look = match style {
        ChipStyle::Head | ChipStyle::CurrentBranch | ChipStyle::Branch if p.dark => dark_local(tone, p, &rows),
        ChipStyle::Head | ChipStyle::CurrentBranch | ChipStyle::Branch => light_local(style, tone, on_trunk, p, &rows),
        ChipStyle::Remote => remote(tone, on_trunk, p, &rows),
        ChipStyle::Tag => ChipLook { fill: None, edge: Some(readable_on(p.tag_fg, p.bg, p.dark, APART)), text: p.tag_fg, halo: None },
    };
    let halo = style.halo().then_some(if p.dark { look.fill } else { look.edge }).flatten();
    ChipLook { halo, ..look }
}

/// A local branch's chip on a dark palette: a fill of the clear tone, lifted toward white until
/// it stands apart from every row and its dark letters read on it; no edge.
fn dark_local(tone: BranchTone, p: &Palette, rows: &[Color32]) -> ChipLook {
    let target = text_target(p);
    let mut fill = apart(clear(tone.chip), rows, true);
    let mut ink = readable_on(p.bg, fill, false, target);
    for _ in 0..MAX_STEPS {
        if ratio(ink, fill) >= target {
            break;
        }
        fill = mix(fill, Color32::WHITE, 0.05);
        ink = readable_on(p.bg, fill, false, target);
    }
    ChipLook { fill: Some(fill), edge: None, text: ink, halo: None }
}

/// A local branch's chip on a light palette: a wash of the hue (a neutral step for HEAD), edged
/// in the line tone pushed apart from every row (HEAD: the trunk tone), lettered in the chip tone
/// (`text` for the trunk and HEAD). An edge that would read brown takes the letters' colour.
fn light_local(style: ChipStyle, tone: BranchTone, on_trunk: bool, p: &Palette, rows: &[Color32]) -> ChipLook {
    let target = text_target(p);
    let (fill, edge) = if style == ChipStyle::Head {
        (mix(p.bg, p.text, 0.12), tone.line)
    } else {
        let (_, c, h) = oklch(tone.chip);
        (from_oklch(PALE.0, c.min(PALE.1), h), apart(clear(tone.line), rows, false))
    };
    let text = if on_trunk {
        readable_on(p.text, fill, false, target)
    } else {
        let ink = readable_on(tone.chip, fill, false, target);
        if brownish(ink) { readable_on(p.text, fill, false, target) } else { ink }
    };
    let edge = if brownish(edge) { text } else { edge };
    ChipLook { fill: Some(fill), edge: Some(edge), text, halo: None }
}

/// A remote branch's chip: no fill; the edge in the line tone (the trunk's own for main) and
/// the text in the chip tone (`text` for main), each pushed apart from every row surface. Text
/// that reads brown or can't reach the target is `text`; a brown edge takes the text's colour.
fn remote(tone: BranchTone, on_trunk: bool, p: &Palette, rows: &[Color32]) -> ChipLook {
    let target = text_target(p);
    let edge = if on_trunk { tone.line } else { apart(clear(tone.line), rows, p.dark) };
    let mut text = if on_trunk { p.text } else { tone.chip };
    let pole = if p.dark { Color32::WHITE } else { Color32::BLACK };
    for _ in 0..MAX_STEPS {
        if worst(text, rows) >= target {
            break;
        }
        text = mix(text, pole, 0.05);
    }
    if brownish(text) || worst(text, rows) < target {
        text = p.text;
    }
    let edge = if brownish(edge) { text } else { edge };
    ChipLook { fill: None, edge: Some(edge), text, halo: None }
}

/// `c`'s least contrast against `rows`.
pub(crate) fn worst(c: Color32, rows: &[Color32]) -> f64 {
    rows.iter().map(|&r| ratio(c, r)).fold(f64::INFINITY, f64::min)
}

/// `c` moved 5% at a time toward white (`dark`) or black until it stands [`APART`] from every
/// one of `rows`.
fn apart(c: Color32, rows: &[Color32], dark: bool) -> Color32 {
    let pole = if dark { Color32::WHITE } else { Color32::BLACK };
    let mut c = c;
    for _ in 0..MAX_STEPS {
        if worst(c, rows) >= APART {
            break;
        }
        c = mix(c, pole, 0.05);
    }
    c
}
