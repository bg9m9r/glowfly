//! A commit row's BRANCH / TAG cell: one soft chip per label (`ref_group::chips`) with its
//! painted marks, as many as fit whole, then a "+N" chip for the rest; the line joining them to
//! the node; which tip the pointer over them asks for; and the ghost "+ branch" chip.

use super::marks::{MARK_W, paint_cloud, paint_local, paint_tag};
use super::paint::Columns;
use super::ref_group::Chip;
use crate::ui::build_status;
use crate::ui::theme::color;
use crate::ui::theme::contrast::{self, ChipStyle, readable_on};
use crate::ui::widgets;
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use trellis_core::forge::CheckState;
use trellis_core::repo::RefKind;

/// Space between the column's edges and its chips.
const PAD: f32 = 6.0;
/// Space between two chips.
const GAP: f32 = 4.0;
const CHIP_H: f32 = 22.0;
/// Space between a chip's edges and its text.
const TEXT_PAD: f32 = 7.0;
/// Room a tag mark takes before a tag's name.
const TAG_LEAD: f32 = 12.0;
/// The least room the first chip keeps beside a "+N" chip; with less, the "+N" chip is dropped.
const MIN_FIRST_W: f32 = 24.0;
const FONT: f32 = 12.5;
/// The "+N" chip's font size and text padding (`widgets::chip`).
const PLUS_FONT: f32 = 11.5;
const PLUS_PAD: f32 = 12.0;

/// What one chip shows.
pub(super) struct ChipSpec {
    text: String,
    style: ChipStyle,
    /// A tag mark before the name.
    lead_tag: bool,
    /// The local mark after the name.
    local: bool,
    /// The remote mark after the name (a remote branch, or a local one's upstream at the same
    /// commit).
    remote: bool,
    /// The build marker at the chip's right end.
    build: Option<CheckState>,
}

impl ChipSpec {
    fn of(chip: &Chip, build: Option<CheckState>) -> Self {
        let l = chip.label;
        let plain = |style, text: String| Self { text, style, lead_tag: false, local: false, remote: false, build };
        match l.kind {
            RefKind::Head => plain(ChipStyle::Head, "HEAD".to_owned()),
            RefKind::Local => Self {
                local: true,
                remote: chip.upstream.is_some(),
                ..plain(if l.is_head { ChipStyle::CurrentBranch } else { ChipStyle::Branch }, check(l.is_head, &l.name))
            },
            RefKind::Remote => Self { remote: true, ..plain(ChipStyle::Remote, l.name.clone()) },
            RefKind::Tag => Self { lead_tag: true, ..plain(ChipStyle::Tag, l.name.clone()) },
        }
    }

    /// How many marks follow the name.
    fn trailing(&self) -> usize {
        usize::from(self.local) + usize::from(self.remote) + usize::from(self.build.is_some())
    }
}

/// The checked-out branch's name with its check mark.
fn check(is_head: bool, name: &str) -> String {
    if is_head { format!("✔ {name}") } else { name.to_owned() }
}

/// A chip's width for its text `text_w` wide: padding, the marks and the build marker.
fn chip_w(spec: &ChipSpec, text_w: f32) -> f32 {
    let lead = if spec.lead_tag { TAG_LEAD } else { 0.0 };
    text_w + 2.0 * TEXT_PAD + lead + MARK_W * spec.trailing() as f32
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
/// chip. The first chip always shows, cut if needed, leaving room for a "+N" chip when there are
/// more (unless the room is too narrow for both). Each later chip shows only whole and only when
/// a "+N" chip for whatever remains still fits after it; the first that doesn't fit stops.
pub(super) fn fit(widths: &[f32], room: f32, plus_w: impl Fn(usize) -> f32, gap: f32) -> Fit {
    let n = widths.len();
    let Some(&want) = widths.first() else { return Fit { shown: 0, first_w: 0.0, first_cut: false, plus: false } };
    let reserve = if n > 1 { plus_w(n - 1) + gap } else { 0.0 };
    if n > 1 && room - reserve < MIN_FIRST_W.min(want) {
        // Too narrow for a "+N" chip beside the first: the first alone, cut to the room.
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

/// Draws a chip of width `w` from `x`, centred on `y`, and returns its rect: a soft fill with no
/// outline, the tag mark, the name (cut to leave its marks room), then the local mark, the remote
/// mark and the build marker.
fn draw_chip(p: &Painter, x: f32, y: f32, spec: &ChipSpec, lane: Color32, w: f32) -> Rect {
    let (fill, fg) = contrast::ref_chip(spec.style, lane, &color::current());
    let r = Rect::from_min_size(pos2(x, y - CHIP_H / 2.0), vec2(w, CHIP_H));
    p.rect_filled(r, CornerRadius::same(4), fill);
    let cp = p.with_clip_rect(r.intersect(p.clip_rect()));
    let mut text_x = x + TEXT_PAD;
    if spec.lead_tag {
        paint_tag(&cp, pos2(x + 9.0, y), fg);
        text_x += TAG_LEAD;
    }
    let marks_left = r.right() - TEXT_PAD - MARK_W * spec.trailing() as f32;
    // The name, ending in "…" when the chip is narrower than it (the half pixel absorbs rounding).
    let g = cut_galley(p.ctx(), cut_job(spec.text.as_str(), FontId::proportional(FONT), fg), marks_left - text_x + 0.5);
    cp.galley(pos2(text_x, y - g.size().y / 2.0), g, fg);
    // Each mark centred a little right of its slot's middle, clear of the name.
    let mut slot = marks_left + MARK_W / 2.0 + 2.0;
    if spec.local {
        paint_local(&cp, pos2(slot, y), fg);
        slot += MARK_W;
    }
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
    /// This drawn chip, which is cut or folds its upstream in.
    One(usize),
}

/// What one row's BRANCH / TAG cell draws.
pub(super) struct RefsRow<'a> {
    pub chips: &'a [Chip<'a>],
    pub lane: Color32,
    /// The tip's build state, shown on the first chip.
    pub build: Option<CheckState>,
    /// The node's left edge, when the GRAPH column shows: the joining line ends there.
    pub node_edge_x: Option<f32>,
    pub pointer: Option<Pos2>,
    /// Show the ghost "+ branch" chip after the labels.
    pub ghost: bool,
}

/// Where a row's BRANCH / TAG cell drew, and what the pointer over it asks for.
#[derive(Default)]
pub(super) struct RefsCell {
    /// From the first chip through the "+N" chip, for the labels menu.
    pub labels_hit: Option<Rect>,
    /// The ghost "+ branch" chip.
    pub ghost: Option<Rect>,
    pub hovered_tip: Option<Tip>,
    /// The pointer is over the drawn chips or the "+N" chip.
    pub on_chip: bool,
}

/// Draws a commit row's BRANCH / TAG cell in `rect` (see the module notes).
pub(super) fn refs_cell(painter: &Painter, cols: &Columns, rect: Rect, row: RefsRow) -> RefsCell {
    let rp = painter.with_clip_rect(cols.rect(cols.refs, rect));
    let mid = rect.center().y;
    let x0 = cols.refs.0 + PAD;
    let mut out = RefsCell::default();
    let mut end = cols.refs.0;
    if !row.chips.is_empty() {
        let specs: Vec<ChipSpec> =
            row.chips.iter().enumerate().map(|(i, c)| ChipSpec::of(c, if i == 0 { row.build } else { None })).collect();
        let text_w = |t: &str, size: f32| rp.layout_no_wrap(t.to_owned(), FontId::proportional(size), color::weak()).size().x;
        let widths: Vec<f32> = specs.iter().map(|s| chip_w(s, text_w(&s.text, FONT))).collect();
        let plus_w = |k: usize| text_w(&format!("+{k}"), PLUS_FONT) + PLUS_PAD;
        let f = fit(&widths, cols.refs.1 - PAD - x0, plus_w, GAP);
        let mut drawn = Vec::with_capacity(f.shown);
        let mut x = x0;
        for (i, spec) in specs.iter().take(f.shown).enumerate() {
            let w = if i == 0 { f.first_w } else { widths[i] };
            drawn.push(draw_chip(&rp, x, mid, spec, row.lane, w));
            x += w + GAP;
        }
        end = drawn.last().map_or(x0, |r| r.right());
        let mut hit = Rect::from_min_max(drawn[0].min, pos2(end, drawn[0].max.y));
        if f.plus {
            let hidden = row.chips.len() - f.shown;
            end += GAP + widgets::chip(&rp, end + GAP, mid, &format!("+{hidden}"), color::toolbar(), color::weak());
            hit.max.x = end;
        }
        out.labels_hit = Some(hit);
        if let Some(edge) = row.node_edge_x {
            let stroke = Stroke::new(1.5_f32, row.lane);
            rp.line_segment([pos2(end, mid), pos2(cols.refs.1, mid)], stroke);
            painter.with_clip_rect(cols.rect(cols.graph, rect)).line_segment([pos2(cols.graph.0, mid), pos2(edge, mid)], stroke);
        }
        if let Some(p) = row.pointer {
            out.on_chip = hit.contains(p);
            out.hovered_tip = if f.plus && out.on_chip {
                Some(Tip::All)
            } else {
                let on = drawn.iter().position(|r| r.contains(p));
                on.filter(|&i| (i == 0 && f.first_cut) || row.chips[i].upstream.is_some()).map(Tip::One)
            };
        }
    }
    // A faded "+ branch" after the checked-out commit's labels: click it to branch from the
    // current branch (other commits: right-click > Create branch here).
    let r = Rect::from_min_size(pos2(end + 6.0, mid - 11.0), vec2(80.0, 22.0));
    if row.ghost && r.right() <= cols.refs.1 - PAD {
        rp.rect_filled(r, CornerRadius::same(3), color::bg());
        rp.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0_f32, row.lane.gamma_multiply(0.6)), StrokeKind::Inside);
        rp.text(pos2(r.left() + 8.0, mid), Align2::LEFT_CENTER, "+ branch", FontId::proportional(12.0), row.lane.gamma_multiply(0.8));
        out.ghost = Some(r);
    }
    out
}

/// Paints one mark centred on a point.
type PaintMark = fn(&Painter, Pos2, Color32);

/// One label in a tooltip: its painted marks, then its name (saying when its upstream is level
/// with it).
pub(super) fn tip_row(ui: &mut Ui, chip: &Chip, lane: Color32) {
    let spec = ChipSpec::of(chip, None);
    let pal = color::current();
    let (_, fg) = contrast::ref_chip(spec.style, lane, &pal);
    let ink = readable_on(fg, ui.visuals().window_fill, pal.dark, 3.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let marks: [(bool, PaintMark); 3] = [(spec.lead_tag, paint_tag), (spec.local, paint_local), (spec.remote, paint_cloud)];
        for (_, paint) in marks.into_iter().filter(|(on, _)| *on) {
            let (r, _) = ui.allocate_exact_size(vec2(MARK_W, MARK_W), Sense::hover());
            paint(ui.painter(), r.center(), ink);
        }
        ui.add_space(2.0);
        ui.label(chip.tip());
    });
}

/// Every label of a row, one per line.
pub(super) fn tip_list(ui: &mut Ui, chips: &[Chip], lane: Color32) {
    for chip in chips {
        tip_row(ui, chip, lane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "+n" chips 30 px wide.
    fn plus(_: usize) -> f32 {
        30.0
    }

    #[test]
    fn every_chip_fits() {
        let f = fit(&[50.0, 40.0, 30.0], 200.0, plus, 4.0);
        assert_eq!(f, Fit { shown: 3, first_w: 50.0, first_cut: false, plus: false });
    }

    #[test]
    fn a_chip_that_doesnt_fit_hides_the_rest() {
        // 50 + 4 + 40 = 94; the third (60) would reach 158 and leave no room for "+2".
        let f = fit(&[50.0, 40.0, 60.0, 20.0], 150.0, plus, 4.0);
        assert_eq!(f, Fit { shown: 2, first_w: 50.0, first_cut: false, plus: true });
        // The last chip needs no "+N" after it.
        assert_eq!(fit(&[50.0, 40.0], 94.0, plus, 4.0).shown, 2);
        assert_eq!(fit(&[50.0, 40.0], 93.0, plus, 4.0), Fit { shown: 1, first_w: 50.0, first_cut: false, plus: true });
    }

    #[test]
    fn a_long_first_chip_is_cut_and_keeps_the_plus() {
        let f = fit(&[300.0, 40.0], 150.0, plus, 4.0);
        assert_eq!(f, Fit { shown: 1, first_w: 116.0, first_cut: true, plus: true });
    }

    #[test]
    fn a_room_narrower_than_the_plus_chip_shows_the_first_chip_only() {
        let f = fit(&[100.0, 40.0, 40.0], 25.0, plus, 4.0);
        assert_eq!(f, Fit { shown: 1, first_w: 25.0, first_cut: true, plus: false });
    }

    #[test]
    fn one_chip_reserves_nothing() {
        assert_eq!(fit(&[140.0], 150.0, plus, 4.0), Fit { shown: 1, first_w: 140.0, first_cut: false, plus: false });
        assert_eq!(fit(&[200.0], 150.0, plus, 4.0), Fit { shown: 1, first_w: 150.0, first_cut: true, plus: false });
        assert_eq!(fit(&[], 150.0, plus, 4.0).shown, 0);
    }

    fn spec(local: bool, remote: bool, build: bool) -> ChipSpec {
        let build = build.then_some(CheckState::Passed);
        ChipSpec { text: String::new(), style: ChipStyle::Branch, lead_tag: false, local, remote, build }
    }

    #[test]
    fn chip_width_counts_its_marks() {
        assert_eq!(chip_w(&spec(false, false, false), 50.0), 64.0, "a name alone");
        assert_eq!(chip_w(&spec(true, false, false), 50.0), 64.0 + MARK_W, "a local branch");
        assert_eq!(chip_w(&spec(true, true, true), 50.0), 64.0 + 3.0 * MARK_W, "in sync, with a build state");
        let tag = ChipSpec { lead_tag: true, ..spec(false, false, false) };
        assert_eq!(chip_w(&tag, 50.0), 64.0 + TAG_LEAD);
        // In a room narrower than the chip the name is cut and the marks keep their room.
        let f = fit(&[chip_w(&spec(true, true, false), 90.0)], 100.0, plus, 4.0);
        assert!(f.first_cut && f.first_w == 100.0, "{f:?}");
    }
}
