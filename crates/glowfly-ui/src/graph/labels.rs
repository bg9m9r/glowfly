//! A commit row's BRANCH / TAG cell: one soft chip per label (`ref_group::chips`) with its
//! painted marks, as many as fit whole, then a "+N" chip for the rest; the line joining them to
//! the node; which tip the pointer over them asks for; where each chip is, for a double-click;
//! and where the New branch… ghost button goes. In a narrow table the same chips lead the
//! message instead (`fit::Fitted::refs_inline`).

use super::marks::{MARK_W, paint_cloud, paint_tag};
use super::ref_group::Chip;
use crate::ui::build_status;
use crate::ui::theme::color;
use crate::ui::theme::contrast::readable_on;
use crate::ui::theme::fonts;
use crate::ui::theme::ref_chip::{self, BranchTone, ChipLook, ChipStyle};
use crate::ui::theme::sizes::{ChipSize, ControlSize, text_pos};
use crate::ui::widgets::chip;
use crate::ui::widgets::truncate::{middle, middle_cut};
use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::forge::CheckState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::RefKind;
use unicode_segmentation::UnicodeSegmentation;

/// Space between two chips.
pub(super) const GAP: f32 = 4.0;
/// A ref chip's size: its height, text padding and text step.
const CHIP: ChipSize = ChipSize::Chip;
/// Space between a chip's edges and its text.
const TEXT_PAD: f32 = CHIP.metrics().pad_x;
/// Room a tag mark takes before a tag's name.
const TAG_LEAD: f32 = 12.0;
/// The characters of the first chip's name it keeps beside a "+N" chip; with less room, the
/// "+N" chip is dropped first.
const NAME_KEEP: usize = 6;
const FONT: f32 = CHIP.metrics().text;
/// How far a halo (`ChipLook::halo`) reaches past its chip: its 1.5 px gap and 1.5 px ring. The
/// halo takes no layout room: it paints into the cell's inset and padding, or the gap after it,
/// and a ref chip after a halo chip moves this much further away (a "+N" badge doesn't).
const HALO_ROOM: f32 = chip::HALO_ROOM;

/// What one chip shows.
pub(super) struct ChipSpec {
    text: String,
    style: ChipStyle,
    /// A tag mark before the name.
    lead_tag: bool,
    /// The remote mark after the name (a remote branch, or a local one with copies on remotes
    /// at the same commit).
    remote: bool,
    /// The build marker at the chip's right end.
    build: Option<CheckState>,
}

impl ChipSpec {
    pub(super) fn of(chip: &Chip, build: Option<CheckState>) -> Self {
        let l = chip.label;
        let plain = |style, text: String| Self { text, style, lead_tag: false, remote: false, build };
        match l.kind {
            RefKind::Head => plain(ChipStyle::Head, "HEAD".to_owned()),
            RefKind::Local => Self {
                remote: chip.folds(),
                ..plain(if l.is_head { ChipStyle::CurrentBranch } else { ChipStyle::Branch }, check(l.is_head, &l.name))
            },
            RefKind::Remote => Self { remote: true, ..plain(ChipStyle::Remote, l.name.clone()) },
            RefKind::Tag => Self { lead_tag: true, ..plain(ChipStyle::Tag, l.name.clone()) },
        }
    }

    /// How many marks follow the name.
    fn trailing(&self) -> usize {
        usize::from(self.remote) + usize::from(self.build.is_some())
    }

    /// The chip's text font: semibold for the checked-out branch, else regular, at the chip's
    /// text size. Every measurement of the chip's text goes through it.
    pub(super) fn font(&self, p: &Painter) -> FontId {
        if self.style == ChipStyle::CurrentBranch { fonts::semibold_in(p.ctx(), FONT) } else { FontId::proportional(FONT) }
    }
}

/// The check mark before the checked-out branch's name.
const CHECK: &str = "✔ ";

/// The checked-out branch's name with its check mark.
fn check(is_head: bool, name: &str) -> String {
    if is_head { format!("{CHECK}{name}") } else { name.to_owned() }
}

/// `spec`'s text as its check mark (the checked-out branch's, else empty) and its name: only
/// the name is ever cut.
fn mark_and_name(spec: &ChipSpec) -> (&str, &str) {
    spec.text.strip_prefix(CHECK).filter(|_| spec.style == ChipStyle::CurrentBranch).map_or(("", &spec.text), |name| (CHECK, name))
}

/// What a chip with `room` for its text shows: its check mark, then its name cut in its middle
/// (`truncate::middle`), so names that share a long start stay apart.
fn chip_text(p: &Painter, spec: &ChipSpec, room: f32) -> String {
    let (mark, name) = mark_and_name(spec);
    let font = spec.font(p);
    let mark_w = if mark.is_empty() { 0.0 } else { text_w(p, mark, &font) };
    format!("{mark}{}", middle(p, name, &font, room - mark_w))
}

/// A chip's width for its text `text_w` wide: padding, the marks and the build marker.
fn chip_w(spec: &ChipSpec, text_w: f32) -> f32 {
    let lead = if spec.lead_tag { TAG_LEAD } else { 0.0 };
    text_w + 2.0 * TEXT_PAD + lead + MARK_W * spec.trailing() as f32
}

/// How far `style`'s halo reaches past its chip's edge (none without a halo).
fn halo_reach(style: ChipStyle) -> f32 {
    if style.halo() { HALO_ROOM } else { 0.0 }
}

/// The clip of chips leading the message in a narrow table, laid out in `span`: the span
/// widened by a halo's reach either side, so a first or last chip's ring isn't cut.
pub(super) fn inline_clip(span: (f32, f32)) -> (f32, f32) {
    (span.0 - HALO_ROOM, span.1 + HALO_ROOM)
}

/// The extra room before each chip: `HALO_ROOM` after a chip with a halo, so its ring doesn't
/// touch the next chip's edge (it reads as a double line), else none.
fn halo_leads(styles: &[ChipStyle]) -> Vec<f32> {
    (0..styles.len()).map(|i| if i > 0 && styles[i - 1].halo() { HALO_ROOM } else { 0.0 }).collect()
}

fn text_w(p: &Painter, text: &str, font: &FontId) -> f32 {
    p.layout_no_wrap(text.to_owned(), font.clone(), color::weak()).size().x
}

/// A chip's whole width for `spec`.
pub(super) fn spec_w(p: &Painter, spec: &ChipSpec) -> f32 {
    chip_w(spec, text_w(p, &spec.text, &spec.font(p)))
}

/// The text a first chip keeps beside a "+N" chip: its check mark, the first `NAME_KEEP`
/// graphemes of its name and "…" (`truncate::middle_cut` counting graphemes, "…" one of them),
/// or the whole text when it is no longer than that.
fn kept_text(spec: &ChipSpec) -> String {
    let (mark, name) = mark_and_name(spec);
    let graphemes = |s: &str| s.graphemes(true).count() as f32;
    middle_cut(name, (NAME_KEEP + 1) as f32, graphemes).map_or_else(|| spec.text.clone(), |(head, _)| format!("{mark}{}…", &name[..head]))
}

/// The width of `spec`'s chip cut to its `kept_text`.
fn kept_w(p: &Painter, spec: &ChipSpec) -> f32 {
    chip_w(spec, text_w(p, &kept_text(spec), &spec.font(p)))
}

/// The "+n" badge.
fn plus_chip(n: usize) -> chip::Chip<'static> {
    chip::Chip::new(ChipSize::Badge, format!("+{n}")).look(chip::Look::new(color::raised(), color::weak()))
}

/// The "+n" badge's width.
pub(super) fn plus_w(p: &Painter, n: usize) -> f32 {
    plus_chip(n).width(p.ctx())
}

/// Which chips fit in a row.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Fit {
    /// How many chips show, from the first.
    pub shown: usize,
    /// The first chip's width, cut to the room when needed.
    pub first_w: f32,
    /// The first chip's name is cut off.
    pub first_cut: bool,
    /// A "+N" chip follows the shown ones.
    pub plus: bool,
}

/// Lays out chips `widths` wide, `gap` apart, in `room`; `plus_w(n)` is the width of the "+n"
/// chip and `first_keep` the first chip's width cut to its first `NAME_KEEP` characters
/// (`kept_w`). The first chip always shows, cut if needed, leaving room for a "+N" chip when
/// there are more; the "+N" chip is dropped first, before the first chip goes below
/// `first_keep`. Each later chip shows only whole and only when a "+N" chip for whatever remains
/// still fits after it; the first that doesn't fit stops.
pub(super) fn fit(widths: &[f32], first_keep: f32, room: f32, plus_w: impl Fn(usize) -> f32, gap: f32) -> Fit {
    let n = widths.len();
    let Some(&want) = widths.first() else { return Fit { shown: 0, first_w: 0.0, first_cut: false, plus: false } };
    let reserve = if n > 1 { plus_w(n - 1) + gap } else { 0.0 };
    if n > 1 && room - reserve < first_keep.min(want) {
        // Too narrow for a "+N" chip beside the first kept chip: the first alone, cut to the room.
        return Fit { shown: 1, first_w: want.min(room).max(0.0), first_cut: want > room, plus: false };
    }
    let first_w = want.min(room - reserve);
    let first_cut = want > room - reserve;
    let mut shown = 1;
    let mut x = first_w;
    while shown < n && !first_cut {
        let w = widths[shown];
        let after = if shown + 1 < n { gap + plus_w(n - shown - 1) } else { 0.0 };
        if x + gap + w + after > room {
            break;
        }
        x += gap + w;
        shown += 1;
    }
    Fit { shown, first_w, first_cut, plus: shown < n }
}

/// A ref chip's box in `r`: its halo (if any), its fill (if any) and its edge 1 px inside it.
fn paint_box(p: &Painter, r: Rect, look: ChipLook) {
    if let Some(halo) = look.halo {
        chip::paint_halo(p, r, CHIP, halo);
    }
    chip::frame(p, r, CHIP, look.into());
}

/// Draws a chip of width `w` from `x`, centred on `y`, and returns its rect: its box and halo
/// (`ref_chip::ref_chip`; the halo reaches outside the rect), the tag mark, the name (cut in
/// its middle to leave its marks room, `chip_text`; semibold for the checked-out branch), then
/// the remote mark and the build marker, coloured from its branch's `tone`.
pub(super) fn draw_chip(p: &Painter, x: f32, y: f32, spec: &ChipSpec, tone: BranchTone, w: f32) -> Rect {
    let look = ref_chip::ref_chip(spec.style, tone, &color::current());
    let fg = look.text;
    let r = chip::rect_at(CHIP, x, y, w.max(0.0));
    paint_box(p, r, look);
    let cp = p.with_clip_rect(r.intersect(p.clip_rect()));
    let mut text_x = x + TEXT_PAD;
    if spec.lead_tag {
        paint_tag(&cp, pos2(x + 9.0, y), fg);
        text_x += TAG_LEAD;
    }
    let marks_left = r.right() - TEXT_PAD - MARK_W * spec.trailing() as f32;
    // The name, cut in its middle when the chip is narrower than it (the half pixel absorbs rounding).
    let g = p.layout_no_wrap(chip_text(p, spec, marks_left - text_x + 0.5), spec.font(p), fg);
    cp.galley(pos2(text_x, text_pos(r, &g).y), g, fg);
    // Each mark centred a little right of its slot's middle, clear of the name.
    let mut slot = marks_left + MARK_W / 2.0 + 2.0;
    if spec.remote {
        paint_cloud(&cp, pos2(slot, y), fg);
        slot += MARK_W;
    }
    if let Some(state) = spec.build {
        build_status::paint_marker_on(&cp, pos2(slot, y), state);
    }
    r
}

/// What the pointer over a row's chips asks to show.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Tip {
    /// Some chips are hidden: every label, one per line.
    All,
    /// This drawn chip, which is cut or folds copies on remotes in.
    One(usize),
}

/// What the pointer over a row's chips asks to show, for chips fitted as `f` out of `n`: every
/// label while any is hidden (with or without a "+N" chip: a dropped "+N" leaves the first chip
/// to say so), else the chip `on` when it is cut or `folds` copies on remotes in.
fn hover_tip(f: Fit, n: usize, on_chip: bool, on: Option<usize>, folds: impl Fn(usize) -> bool) -> Option<Tip> {
    if on_chip && f.shown < n {
        return Some(Tip::All);
    }
    on.filter(|&i| (i == 0 && f.first_cut) || folds(i)).map(Tip::One)
}

/// What one row's BRANCH / TAG cell draws.
pub(super) struct RefsRow<'a> {
    pub chips: &'a [Chip<'a>],
    /// Each chip's branch tones (`lane_colours::chip_tones`).
    pub tones: &'a [BranchTone],
    /// The node's colour: the joining line.
    pub lane: Color32,
    /// The tip's build state, shown on the first chip.
    pub build: Option<CheckState>,
    /// The node's left edge, when the GRAPH column shows: the joining line ends there.
    pub node_edge_x: Option<f32>,
    pub pointer: Option<Pos2>,
    /// Leave room for the New branch… ghost button after the labels.
    pub ghost: bool,
    /// The ghost button's width (`ghost::ghost_width` at Compact).
    pub ghost_w: f32,
}

/// Where a row's BRANCH / TAG cell drew, and what the pointer over it asks for.
#[derive(Default)]
pub(super) struct RefsCell {
    /// From the first chip through the "+N" chip, for the labels menu.
    pub labels_hit: Option<Rect>,
    /// Where the New branch… ghost button goes, when it shows and fits, and whether its label
    /// fits too (else it is its plus alone).
    pub ghost_at: Option<(Rect, bool)>,
    pub hovered_tip: Option<Tip>,
    /// The pointer is over the drawn chips or the "+N" chip.
    pub on_chip: bool,
    /// Each drawn chip's rect, by its index in `RefsRow::chips`; hidden chips and the "+N" chip
    /// have none.
    pub chip_rects: Vec<Rect>,
}

impl RefsCell {
    /// The drawn chip under `p`, by its index in the row's chips.
    pub(super) fn chip_at(&self, p: Pos2) -> Option<usize> {
        self.chip_rects.iter().position(|r| r.contains(p))
    }
}

/// Draws a commit row's chips in `rect` (see the module notes): from `span.0` to `span.1` at
/// most, clipped to `clip` (BRANCH / TAG's content and cell, or a span leading the message).
pub(super) fn refs_cell(painter: &Painter, span: (f32, f32), clip: (f32, f32), rect: Rect, row: RefsRow) -> RefsCell {
    let rp = painter.with_clip_rect(Rect::from_x_y_ranges(clip.0..=clip.1.max(clip.0), rect.y_range()));
    let mid = rect.center().y;
    let x0 = span.0;
    let mut out = RefsCell::default();
    let mut end = span.0;
    let mut reach = span.0;
    if !row.chips.is_empty() {
        let specs: Vec<ChipSpec> =
            row.chips.iter().enumerate().map(|(i, c)| ChipSpec::of(c, if i == 0 { row.build } else { None })).collect();
        let leads = halo_leads(&specs.iter().map(|s| s.style).collect::<Vec<_>>());
        let widths: Vec<f32> = specs.iter().zip(&leads).map(|(s, lead)| spec_w(&rp, s) + lead).collect();
        let f = fit(&widths, kept_w(&rp, &specs[0]), span.1 - x0, |k| plus_w(&rp, k), GAP);
        let mut drawn = Vec::with_capacity(f.shown);
        let mut x = x0;
        for (i, spec) in specs.iter().take(f.shown).enumerate() {
            let w = if i == 0 { f.first_w } else { widths[i] };
            let tone = row.tones.get(i).copied().unwrap_or(BranchTone::plain(row.lane));
            drawn.push(draw_chip(&rp, x + leads[i], mid, spec, tone, w - leads[i]));
            x += w + GAP;
        }
        end = drawn.last().map_or(x0, |r| r.right());
        // How far the drawing reaches: past the last chip's halo, when it has one.
        reach = end + specs.get(f.shown - 1).map_or(0.0, |s| halo_reach(s.style));
        let mut hit = Rect::from_min_max(drawn[0].min, pos2(reach, drawn[0].max.y));
        if f.plus {
            let hidden = row.chips.len() - f.shown;
            end = plus_chip(hidden).paint(&rp, end + GAP, mid).right();
            reach = end;
            hit.max.x = end;
        }
        out.labels_hit = Some(hit);
        out.chip_rects = drawn;
        if let Some(p) = row.pointer {
            out.on_chip = hit.contains(p);
            out.hovered_tip = hover_tip(f, row.chips.len(), out.on_chip, out.chip_at(p), |i| row.chips[i].folds());
        }
    }
    // Room for the New branch… ghost button after the checked-out commit's labels (the graph
    // draws it there; other commits: right-click > Create branch here): labelled, or its plus
    // alone where the label doesn't fit.
    if row.ghost {
        let at = |w: f32| Rect::from_min_size(pos2(end + GAP, mid - ghost_h() / 2.0), vec2(w, ghost_h()));
        out.ghost_at =
            [(row.ghost_w, true), (ghost_h(), false)].into_iter().map(|(w, labelled)| (at(w), labelled)).find(|(r, _)| r.right() <= span.1);
    }
    // The line joining the labels (and the ghost button after them) to the node.
    if let (Some(edge), false) = (row.node_edge_x, row.chips.is_empty()) {
        let from = out.ghost_at.map_or(reach, |(r, _)| r.right());
        painter.line_segment([pos2(from, mid), pos2(edge, mid)], Stroke::new(1.5_f32, row.lane));
    }
    out
}

/// Paints one mark centred on a point.
type PaintMark = fn(&Painter, Pos2, Color32);

/// One label in a tooltip: its painted marks, then its name (naming the copies on remotes level
/// with it), its marks in its branch's `tone`.
pub(super) fn tip_row(ui: &mut Ui, t: Lang, chip: &Chip, tone: BranchTone) {
    let spec = ChipSpec::of(chip, None);
    let pal = color::current();
    // The branch's colour: a dark palette's filled chip carries it in its fill, not its letters.
    let look = ref_chip::ref_chip(spec.style, tone, &pal);
    let fg = match look.fill {
        Some(fill) if pal.dark => fill,
        _ => look.text,
    };
    let ink = readable_on(fg, ui.visuals().window_fill, pal.dark, 3.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let marks: [(bool, PaintMark); 2] = [(spec.lead_tag, paint_tag), (spec.remote, paint_cloud)];
        for (_, paint) in marks.into_iter().filter(|(on, _)| *on) {
            let (r, _) = ui.allocate_exact_size(vec2(MARK_W, MARK_W), Sense::hover());
            paint(ui.painter(), r.center(), ink);
        }
        ui.add_space(2.0);
        ui.label(chip.tip(t));
    });
}

/// Every label of a row, one per line, each in its own `tones`.
pub(super) fn tip_list(ui: &mut Ui, t: Lang, chips: &[Chip], tones: &[BranchTone]) {
    for (chip, tone) in chips.iter().zip(tones) {
        tip_row(ui, t, chip, *tone);
    }
}

/// The New branch… ghost button's height: a Compact control's.
fn ghost_h() -> f32 {
    ControlSize::Compact.metrics().h
}

#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;
