//! The conflict view's ours and theirs panes side by side: a Take checkbox per side in each
//! chunk's band (ui/conflict_band.rs) and one per line of a chunk, in the side's colour, so a
//! chunk can take whole sides or single lines.
//! Long clean runs fold into one row (ui/conflict_fold.rs). The output below them is in
//! ui/conflict_output.rs; the cells are painted by ui/conflict_cells.rs.

use super::conflict_band::band;
use super::conflict_cells::{CHECK_W, cell, conflict_wrap, content_width, display, offsets, side_color, tint, visual_lines, widest};
use super::conflict_fold::{fold_back, fold_row};
use super::diff::ROW_H;
use super::diff::wrap::{RowMetrics, Wrap};
use super::theme::color;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::conflict::ConflictView;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{Align, Color32, Direction, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use glowfly_core::conflict_folds::clean_fold;
use glowfly_core::conflict_rows::{PaneRow, pane_rows, scroll_target};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Chunk, ConflictFile, Segment, Side};

/// A line checkbox's hit area's width, from the pane's edge; it is as tall as the row, so a click
/// always goes to the row under the pointer. Its painted box stays 14 px.
const HIT_W: f32 = 24.0;

/// The branch (or commit) name of `side`, for the checkboxes' and Take all's spoken names.
pub(super) fn side_name(t: Lang, side: Side, labels: Option<&glowfly_core::repo::SideLabels>) -> String {
    match (side, labels) {
        (Side::Ours, Some(l)) => l.ours.clone(),
        (Side::Theirs, Some(l)) => l.theirs.clone(),
        (Side::Ours, None) => t.tr("conflict.panes.ours").to_owned(),
        (Side::Theirs, None) => t.tr("conflict.panes.theirs").to_owned(),
    }
}

/// How many visual lines pane row `row` takes: a chunk row the taller of its two sides; headers
/// and folds one.
fn row_lines(row: &PaneRow, segs: &[Segment], chunks: &[&Chunk], wrap: Option<Wrap>) -> usize {
    match *row {
        PaneRow::Clean { seg, line, .. } => match &segs[seg] {
            Segment::Clean(lines) => visual_lines(&display(&lines[line]), wrap),
            Segment::Conflict(_) => 1,
        },
        PaneRow::Lines { chunk, row, .. } => {
            let side = |s: Side| chunks[chunk].side(s).get(row).map_or(1, |l| visual_lines(&display(l), wrap));
            side(Side::Ours).max(side(Side::Theirs))
        }
        PaneRow::Fold(_) | PaneRow::Header(_) => 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn panes(
    ui: &mut Ui,
    t: Lang,
    view: &ConflictView,
    file: &ConflictFile,
    segs: &[Segment],
    scroll_to: Option<usize>,
    wrap_on: bool,
    msgs: &mut Vec<Msg>,
) {
    let (rows, firsts) = pane_rows(segs, &view.opened);
    let chunks: Vec<&Chunk> = file.chunks().collect();
    let half_view = (ui.available_width() / 2.0).round();
    // Column headers: the side, its name and Take all.
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, color::raised());
    let ours = Rect::from_min_max(strip.min, pos2(strip.left() + half_view, strip.bottom()));
    let theirs = Rect::from_min_max(pos2(strip.left() + half_view, strip.top()), strip.max);
    for (side, title, take_tip, name, r) in [
        (Side::Ours, t.tr("conflict.panes.ours"), t.tr("conflict.panes.take_all.ours.tip"), &file.labels.ours, ours),
        (Side::Theirs, t.tr("conflict.panes.theirs"), t.tr("conflict.panes.take_all.theirs.tip"), &file.labels.theirs, theirs),
    ] {
        let builder = UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center));
        ui.allocate_new_ui(builder, |ui| {
            ui.label(type_scale::section_text(ui, title));
            ui.label(RichText::new(name).color(color::weak()));
            if file.no_ancestor {
                ui.label(RichText::new(t.tr("conflict.panes.no_base")).size(type_scale::SMALL).color(color::faint()));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let all = ui.small_button(t.tr("conflict.panes.take_all.button"));
                let spoken = match side {
                    Side::Ours => t.trf("conflict.panes.take_all.ours.name", &[("branch", name)]),
                    Side::Theirs => t.trf("conflict.panes.take_all.theirs.name", &[("branch", name)]),
                };
                crate::ui::access::name_button(&all, true, &spoken);
                if all.on_hover_text(take_tip).clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictTakeAll(side)));
                }
            });
        });
    }
    let wrap = conflict_wrap(ui, wrap_on, 2.0, CHECK_W);
    let width = content_width(ui, widest(segs), 2.0, CHECK_W, wrap.is_some());
    let heights;
    let m = match wrap {
        Some(w) => {
            heights = offsets(rows.len(), w, |i| row_lines(&rows[i], segs, &chunks, wrap));
            RowMetrics::Varied(&heights)
        }
        None => RowMetrics::Uniform(ROW_H),
    };
    super::diff::wrap::scroll(ui, "conflict-panes", rows.len(), m, scroll_to.map(|k| scroll_target(&firsts, k)), |ui, i, h| {
        // Wrapped, the rows are as wide as the panel.
        let width = if wrap.is_some() { ui.available_width() } else { width };
        let half = (width / 2.0).round();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
        let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
        let right = Rect::from_min_max(pos2(left.right(), rect.top()), rect.max);
        match rows[i] {
            PaneRow::Clean { seg, line, ours_no, theirs_no } => {
                let Segment::Clean(lines) = &segs[seg] else { return };
                let text = display(&lines[line]);
                let ink = (Color32::TRANSPARENT, color::text());
                cell(ui, left, CHECK_W, Some(ours_no), &text, ink, wrap);
                cell(ui, right, CHECK_W, Some(theirs_no), &text, ink, wrap);
                // The first line of an opened fold folds it back.
                if let Some(f) = clean_fold(segs, seg).filter(|f| f.start == line && view.opened.contains(&seg)) {
                    fold_back(ui, t, left, f, msgs);
                }
            }
            PaneRow::Fold(f) => {
                fold_row(ui, t, rect, f, msgs);
                return;
            }
            PaneRow::Header(k) => {
                for (side, r) in [(Side::Ours, left), (Side::Theirs, right)] {
                    band(ui, t, r, view, k, side, msgs);
                }
                if resp.clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(k)));
                }
            }
            PaneRow::Lines { chunk, row, ours, theirs } => {
                let c = chunks[chunk];
                for (side, r, number) in [(Side::Ours, left, ours), (Side::Theirs, right, theirs)] {
                    let lines = c.side(side);
                    match number {
                        Some(n) => {
                            let pick = &view.picks[chunk];
                            // Lines a picked chunk leaves out are dimmed.
                            let left_out = !pick.is_open() && !pick.is_edited() && !pick.takes_line(c, side, row);
                            let bg = if left_out { tint(side).gamma_multiply(0.5) } else { tint(side) };
                            cell(ui, r, CHECK_W, Some(n), &display(&lines[row]), (bg, color::text()), wrap);
                            line_check(ui, t, r, view, c, (chunk, side, row, n), msgs);
                        }
                        None if lines.is_empty() && row == 0 => {
                            let bg = tint(side).gamma_multiply(0.5);
                            cell(ui, r, CHECK_W, None, t.tr("conflict.panes.no_lines"), (bg, color::faint()), None)
                        }
                        None => cell(ui, r, CHECK_W, None, "", (color::panel(), color::text()), None),
                    }
                }
                if resp.clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(chunk)));
                }
            }
        }
        ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
    });
}

/// The checkbox at the left of a chunk line's cell `r`, on its first visual line: takes or
/// leaves out line `line` (numbered `number` in the file) of `side` in chunk `k`. Its 14 px box
/// sits in a hit area 24 px wide and as tall as the row. Off once the chunk's text was typed in the output.
fn line_check(
    ui: &mut Ui,
    t: Lang,
    r: Rect,
    view: &ConflictView,
    c: &Chunk,
    (k, side, line, number): (usize, Side, usize, u32),
    msgs: &mut Vec<Msg>,
) {
    let pick = &view.picks[k];
    let mut taken = pick.takes_line(c, side, line);
    let rect = Rect::from_min_max(r.min, pos2(r.left() + HIT_W, r.top() + ROW_H));
    // The box: centred in the gutter right of its 2 px margin, on whole pixels so it stays crisp.
    let at = pos2(r.left() + 1.0 + CHECK_W / 2.0, r.top() + ROW_H / 2.0).round();
    let mut builder = UiBuilder::new().max_rect(rect).layout(Layout::centered_and_justified(Direction::TopDown));
    if pick.is_edited() {
        builder = builder.disabled();
    }
    // A child that doesn't move the rows' cursor; added after the row, so it gets the click.
    let mut child = ui.new_child(builder);
    let tip = if taken { t.tr("conflict.panes.leave_line.tip") } else { t.tr("conflict.panes.take_line.tip") };
    let branch = side_name(t, side, view.file.as_ref().map(|f| &f.labels));
    let args: [(&str, &dyn std::fmt::Display); 3] = [("line", &number), ("branch", &branch), ("number", &(k + 1))];
    let name = match side {
        Side::Ours => t.trf("conflict.panes.take_line.ours.name", &args),
        Side::Theirs => t.trf("conflict.panes.take_line.theirs.name", &args),
    };
    let check = child.add(controls::Check::without_text(&mut taken).tint(side_color(side)).hit(rect.size()).box_at(at).name(name));
    let resp = check.on_hover_text(tip).on_disabled_hover_text(t.tr("conflict.panes.edited.tip"));
    if resp.changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggleLine { chunk: k, side, line }));
    }
}
