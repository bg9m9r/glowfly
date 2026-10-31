//! The conflict view's ours and theirs panes side by side: a Take checkbox per side in each
//! chunk header and one per line of a chunk, so a chunk can take whole sides or single lines.
//! Long clean runs fold into one row (ui/conflict_fold.rs). The output below them is in
//! ui/conflict_output.rs; the cells are painted by ui/conflict_cells.rs.

use super::conflict_cells::{CHECK_W, cell, conflict_wrap, content_width, current_bar, display, offsets, tint, visual_lines, widest};
use super::conflict_fold::{fold_back, fold_row};
use super::diff::wrap::{RowMetrics, Wrap};
use super::diff::{ROW_H, mono};
use super::theme::color;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::conflict::ConflictView;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Align2, Color32, Direction, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use glowfly_core::conflict_folds::clean_fold;
use glowfly_core::conflict_rows::{PaneRow, pane_rows, scroll_target};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Chunk, ConflictFile, Segment, Side};

/// The width of a chunk header's Take checkbox and of its Discard edit button, for their
/// English text; a longer translation widens them by the difference.
const TAKE_W: f32 = 64.0;
const DISCARD_W: f32 = 90.0;

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
                if ui.small_button(t.tr("conflict.panes.take_all.button")).on_hover_text(take_tip).clicked() {
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
                    chunk_header(ui, t, r, view, k, side, msgs);
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
                            line_check(ui, t, r, view, c, (chunk, side, row), msgs);
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
/// leaves out line `line` of `side` in chunk `k`. Off once the chunk's text was typed in the
/// output.
fn line_check(ui: &mut Ui, t: Lang, r: Rect, view: &ConflictView, c: &Chunk, (k, side, line): (usize, Side, usize), msgs: &mut Vec<Msg>) {
    let pick = &view.picks[k];
    let mut taken = pick.takes_line(c, side, line);
    let rect = Rect::from_min_max(pos2(r.left() + 2.0, r.top()), pos2(r.left() + CHECK_W, r.top() + ROW_H));
    let mut builder = UiBuilder::new().max_rect(rect).layout(Layout::centered_and_justified(Direction::TopDown));
    if pick.is_edited() {
        builder = builder.disabled();
    }
    // A child that doesn't move the rows' cursor; added after the row, so it gets the click.
    let mut child = ui.new_child(builder);
    let tip = if taken { t.tr("conflict.panes.leave_line.tip") } else { t.tr("conflict.panes.take_line.tip") };
    let check = child.add(controls::Check::without_text(&mut taken));
    let resp = check.on_hover_text(tip).on_disabled_hover_text(t.tr("conflict.panes.edited.tip"));
    if resp.changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggleLine { chunk: k, side, line }));
    }
}

/// A chunk's header in one column: "Conflict N" (with a faint tag when its pick was made
/// automatically or its text typed in the output) and a Take checkbox for that side (ticked
/// when every line of it is taken, mixed when some are). A chunk edited in the output has
/// Discard edit in the theirs column instead.
fn chunk_header(ui: &mut Ui, t: Lang, r: Rect, view: &ConflictView, k: usize, side: Side, msgs: &mut Vec<Msg>) {
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    p.rect_filled(r, 0.0, color::hunk_bg());
    let name = t.trf("conflict.panes.chunk.title", &[("number", &(k + 1))]);
    let title = p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, name, mono(), color::hunk_fg());
    if k == view.current {
        current_bar(ui, r);
    }
    let (Some(pick), Some(c)) = (view.picks.get(k), view.chunk(k)) else { return };
    let edited = pick.is_edited();
    // Kept in view when the panes scroll sideways.
    let x = r.right().min(ui.clip_rect().right()) - 8.0;
    let (label, english) = if edited {
        (t.tr("conflict.panes.discard_edit.button"), Lang::En.tr("conflict.panes.discard_edit.button"))
    } else {
        (t.tr("conflict.panes.take.check"), Lang::En.tr("conflict.panes.take.check"))
    };
    let text_w =
        |s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_owned(), egui::FontId::proportional(type_scale::BODY), color::text()).size().x);
    let grow = (text_w(label) - text_w(english)).max(0.0);
    let w = if edited { DISCARD_W } else { TAKE_W } + grow;
    let check = Rect::from_min_max(pos2(x - w, r.top()), pos2(x, r.bottom()));
    let tag = match view.auto.get(k).copied().flatten() {
        _ if edited => Some(t.tr("conflict.panes.edited.tag").to_owned()),
        Some(why) => Some(t.trf("conflict.panes.auto.tag", &[("why", &why.label(t))])),
        None => None,
    };
    if let Some(tag) = tag {
        // Cut short before the checkbox rather than run under it.
        let room = Rect::from_min_max(pos2(title.right(), r.top()), pos2(check.left() - 8.0, r.bottom()));
        p.with_clip_rect(room.intersect(p.clip_rect())).text(
            pos2(title.right() + 12.0, r.center().y),
            Align2::LEFT_CENTER,
            tag,
            mono(),
            color::faint(),
        );
    }
    if edited {
        if side == Side::Theirs {
            let b = ui.put(check, egui::Button::new(RichText::new(label).size(type_scale::SMALL)));
            if b.on_hover_text(t.tr("conflict.panes.discard_edit.tip")).clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictDiscardEdit(k)));
            }
        }
        return;
    }
    let mut taken = pick.takes_side(c, side);
    let check_box = controls::Check::new(&mut taken, label).indeterminate(pick.partly_takes_side(c, side));
    if ui.put(check, check_box).changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggle { chunk: k, side }));
    }
}
