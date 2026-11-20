//! The unchanged runs' controls: the whole-file view's "N unchanged lines" row of a folded run,
//! which opens in place when clicked, the hunk view's unfold control on a hunk header, which fills
//! in the unchanged lines above it, the same kind of row below the last hunk, and the fold-back
//! control on the first line of an opened run (in either view), which folds it again. While an
//! opened run's text is being read, its control shows a spinner in the chevrons' place.

use super::DiffView;
use crate::theme::color;
use crate::theme::tokens::radius;
use crate::widgets::paint_text;
use eframe::egui::{Align2, CornerRadius, CursorIcon, Painter, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::diff_rows::GapSpan;
use glowfly_core::whole_file::Fold;
use glowfly_model::msg::DiffMsg;
use glowfly_model::msg::Msg;

/// The row's text for a fold of `n` lines.
fn label(v: &DiffView, n: usize) -> String {
    v.t.trn("diff.fold.lines", n, &[])
}

/// What a full-width run row says and does: its id salt, accessible name, visible text, the
/// hover text when it can be clicked, and whether its lines are being read (then it shows a
/// spinner, and can't be clicked).
struct RunRow<'a> {
    salt: (&'static str, u32),
    name: &'a str,
    text: &'a str,
    tip: &'a str,
    waiting: bool,
}

/// Draws a run row across the full row `rect`: a quiet fill, the unfold mark in the sign column
/// (at `sign_x` from the row's left) or a spinner in its place, and the text in the text column
/// (at `text_x`), pinned to the visible left edge when scrolled sideways. Returns whether it was
/// clicked.
fn run_row(ui: &mut Ui, rect: Rect, v: &DiffView, row: RunRow, sign_x: f32, text_x: f32) -> bool {
    let sense = if row.waiting { Sense::hover() } else { Sense::click() };
    let resp = ui.interact(rect, ui.id().with(row.salt), sense);
    let kind = if row.waiting { WidgetType::ProgressIndicator } else { WidgetType::Button };
    resp.widget_info(|| WidgetInfo::labeled(kind, true, row.name));
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let hover = resp.hovered() && !row.waiting;
    p.rect_filled(rect, 0.0, if hover { color::hover() } else { color::panel() });
    let hairline = Stroke::new(1.0_f32, color::border());
    p.hline(rect.x_range(), rect.top() + 0.5, hairline);
    p.hline(rect.x_range(), rect.bottom() - 0.5, hairline);

    // The unfold mark: chevrons pointing apart.
    let k = v.style.k;
    let c = pos2(rect.left() + sign_x + 4.0 * k, rect.center().y);
    if row.waiting {
        spinner(ui, Rect::from_center_size(c, vec2(12.0 * k, 12.0 * k)));
    } else {
        paint_chevrons(&p, c, k, Stroke::new(1.4_f32, color::weak()), true);
    }

    let x = (rect.left() + text_x).max(ui.clip_rect().left() + 8.0);
    paint_text(&p, pos2(x, rect.center().y), Align2::LEFT_CENTER, row.text, v.style.font.clone(), color::weak());

    if hover {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    !row.waiting && resp.on_hover_text(row.tip).clicked()
}

/// The loading mark of a control: a small spinner in `rect`, in the quiet ink.
fn spinner(ui: &Ui, rect: Rect) {
    crate::widgets::Spinner::new().size(rect.width()).color(color::weak()).paint_at(ui, rect);
}

/// Draws fold `f` across the full row `rect`. A click asks to show the lines.
pub(super) fn fold_row(ui: &mut Ui, rect: Rect, v: &DiffView, f: Fold, sign_x: f32, text_x: f32, msgs: &mut Vec<Msg>) {
    let text = label(v, f.len());
    let row = RunRow { salt: ("diff-fold", f.key), name: &text, text: &text, tip: v.t.tr("diff.fold.show.tip"), waiting: false };
    if run_row(ui, rect, v, row, sign_x, text_x) {
        msgs.push(Msg::Diff(DiffMsg::ExpandUnchanged(f.key)));
    }
}

/// The hunk view's row for the closed run `g` below the last hunk, drawn like the whole-file
/// view's fold row, with the unfold mark and "N unchanged lines" (the reading text and a spinner
/// while its lines are being read). A click asks to show the lines. During a line drag the row
/// counts as past the last hunk's last line.
pub(super) fn gap_below_row(ui: &mut Ui, rect: Rect, v: &DiffView, g: GapSpan, sign_x: f32, text_x: f32, msgs: &mut Vec<Msg>) {
    super::drag_over(ui, v, rect, g.hunk, 0, msgs);
    let waiting = v.waiting.contains(&g.key);
    let (name, text) = if waiting {
        (v.t.tr("diff.gap.reading").to_owned(), v.t.tr("diff.gap.reading").to_owned())
    } else {
        (v.t.trn("diff.gap.below", g.len, &[]), label(v, g.len))
    };
    let row = RunRow { salt: ("diff-gap", g.key), name: &name, text: &text, tip: v.t.tr("diff.fold.show.tip"), waiting };
    if run_row(ui, rect, v, row, sign_x, text_x) {
        msgs.push(Msg::Diff(DiffMsg::ExpandUnchanged(g.key)));
    }
}

/// A small chevron above one below, centred on `c` at scale `k`: tips pointing away from the
/// centre when `apart` (unfold), towards it otherwise (fold back). Painted, since the bundled
/// fonts lack such glyphs.
pub(crate) fn paint_chevrons(p: &Painter, c: Pos2, k: f32, stroke: Stroke, apart: bool) {
    let (tip, arms) = if apart { (4.5, 1.5) } else { (1.5, 4.5) };
    for dir in [-1.0, 1.0] {
        let t = pos2(c.x, c.y + dir * tip * k);
        for side in [-1.0, 1.0] {
            p.line_segment([t, pos2(c.x + side * 3.5 * k, c.y + dir * arms * k)], stroke);
        }
    }
}

/// The opened fold that starts at whole-file line `line`, if any. `opened` is in file order.
pub(super) fn opened_at(opened: &[Fold], line: usize) -> Option<Fold> {
    opened.binary_search_by_key(&line, |f| f.start).ok().map(|i| opened[i])
}

/// The hunk view's unfold control at the left edge of the header row `rect` of the hunk after
/// run `g` (the first hunk's for the run above it): chevrons pointing apart, highlighted on
/// hover, a spinner while the run's lines are being read. A click fills in the run's lines.
pub(super) fn gap_control(ui: &mut Ui, rect: Rect, v: &DiffView, g: GapSpan, msgs: &mut Vec<Msg>) {
    let k = v.style.k;
    let r = Rect::from_center_size(pos2(rect.left() + 8.0 + 7.0 * k, rect.center().y), vec2(14.0 * k, v.style.row_h - 2.0));
    let waiting = v.waiting.contains(&g.key);
    let text = if waiting { v.t.tr("diff.gap.reading").to_owned() } else { v.t.trn("diff.gap.above", g.len, &[]) };
    let resp = ui.interact(r, ui.id().with(("diff-gap", g.key)), if waiting { Sense::hover() } else { Sense::click() });
    let kind = if waiting { WidgetType::ProgressIndicator } else { WidgetType::Button };
    resp.widget_info(|| WidgetInfo::labeled(kind, true, &text));
    if waiting {
        spinner(ui, Rect::from_center_size(r.center(), vec2(12.0 * k, 12.0 * k)));
        resp.on_hover_text(text);
        return;
    }
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let hovered = resp.hovered();
    if hovered {
        p.rect_filled(r, CornerRadius::same(radius::SM), color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if hovered { color::text() } else { color::weak() };
    paint_chevrons(&p, r.center(), k, Stroke::new(1.4_f32, ink), true);
    if resp.on_hover_text(text).clicked() {
        msgs.push(Msg::Diff(DiffMsg::ExpandUnchanged(g.key)));
    }
}

/// The fold-back control on the first line of opened fold `f`: `fold_back_key` for it.
pub(super) fn fold_back(ui: &mut Ui, row: Rect, v: &DiffView, f: Fold, sign_x: f32, msgs: &mut Vec<Msg>) {
    fold_back_key(ui, row, v, f.key, f.len(), sign_x, msgs);
}

/// The fold-back control on the first line of the opened run `key` of `len` lines, in the sign
/// column (at `sign_x` from the left of `row`, where the fold row paints its unfold mark):
/// chevrons pointing together, highlighted on hover. A click folds the run again.
pub(super) fn fold_back_key(ui: &mut Ui, row: Rect, v: &DiffView, key: u32, len: usize, sign_x: f32, msgs: &mut Vec<Msg>) {
    let k = v.style.k;
    // On the row's first visual line, when its line wraps.
    let y = row.top() + v.style.row_h / 2.0;
    let r = Rect::from_center_size(pos2(row.left() + sign_x + 4.0 * k, y), vec2(14.0 * k, v.style.row_h - 2.0));
    let resp = ui.interact(r, ui.id().with(("diff-fold-back", key)), Sense::click());
    let text = v.t.trn("diff.fold.back", len, &[]);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &text));
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let hovered = resp.hovered();
    if hovered {
        p.rect_filled(r, CornerRadius::same(radius::SM), color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if hovered { color::text() } else { color::faint() };
    paint_chevrons(&p, r.center(), k, Stroke::new(1.4_f32, ink), false);
    let tip = v.t.trn("diff.fold.back.tip", len, &[]);
    if resp.on_hover_text(tip).clicked() {
        msgs.push(Msg::Diff(DiffMsg::CollapseUnchanged(key)));
    }
}

#[cfg(test)]
mod tests {
    use glowfly_core::i18n::Lang;
    use glowfly_core::whole_file::Fold;

    #[test]
    fn labels_count_lines_in_every_form() {
        let t = Lang::En;
        assert_eq!(t.trn("diff.fold.lines", 1, &[]), "1 unchanged line");
        assert_eq!(t.trn("diff.fold.lines", 11, &[]), "11 unchanged lines");
        assert_eq!(t.trn("diff.gap.above", 1, &[]), "Show the 1 unchanged line above");
        assert_eq!(t.trn("diff.gap.above", 7, &[]), "Show the 7 unchanged lines above");
        assert_eq!(t.trn("diff.gap.below", 1, &[]), "Show the 1 unchanged line below");
        assert_eq!(t.trn("diff.gap.below", 5, &[]), "Show the 5 unchanged lines below");
        assert_eq!(t.trn("diff.fold.back", 1, &[]), "Fold 1 unchanged line");
        assert_eq!(t.trn("diff.fold.back", 4, &[]), "Fold 4 unchanged lines");
        assert_eq!(t.trn("diff.fold.back.tip", 1, &[]), "Fold this unchanged line back into one row");
        assert_eq!(t.trn("diff.fold.back.tip", 4, &[]), "Fold these 4 unchanged lines back into one row");
        assert_eq!((t.tr("diff.fold.show.tip"), t.tr("diff.gap.reading")), ("Show these lines", "Reading the unchanged lines…"));
    }

    #[test]
    fn opened_at_finds_folds_by_start() {
        let f = |start: usize| Fold { start, end: start + 5, key: start as u32 };
        let opened = [f(2), f(20)];
        assert_eq!(super::opened_at(&opened, 20), Some(f(20)));
        assert_eq!(super::opened_at(&opened, 21), None, "only the first line");
        assert_eq!(super::opened_at(&opened, 3), None);
        assert_eq!(super::opened_at(&[], 2), None);
    }
}
