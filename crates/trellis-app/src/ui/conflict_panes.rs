//! The conflict view's ours and theirs panes side by side: a Take checkbox per side in each
//! chunk header and one per line of a chunk, so a chunk can take whole sides or single lines.
//! The output below them is in ui/conflict_output.rs.

use super::diff::{ROW_H, mono};
use super::theme::color;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::conflict::ConflictView;
use eframe::egui::{self, Align, Align2, Color32, Direction, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use trellis_core::conflict_rows::{PaneRow, pane_rows, scroll_target};
use trellis_core::repo::{Chunk, ConflictFile, Segment, Side};

/// Line numbers end here; text starts at `TEXT_X` (both after a cell's inset).
pub(super) const GUTTER: f32 = 40.0;
pub(super) const TEXT_X: f32 = 50.0;
/// The panes' inset for the line checkboxes, on every row so numbers line up.
const CHECK_W: f32 = 18.0;

/// The disabled tooltip of a chunk's checkboxes once its text was typed in the output.
const EDITED_TIP: &str = "Edited in the output: Discard edit brings the picks back";

/// Trims the terminator and shows tabs as 4 spaces (the file is written unchanged).
pub(super) fn display(line: &str) -> String {
    line.trim_end_matches('\n').trim_end_matches('\r').replace('\t', "    ")
}

pub(super) fn tint(side: Side) -> Color32 {
    match side {
        Side::Ours => color::blue().gamma_multiply(0.18),
        Side::Theirs => color::purple().gamma_multiply(0.18),
    }
}

/// One cell: background, line number and text (`inset` from its left edge), clipped to its
/// column.
pub(super) fn cell(ui: &Ui, rect: Rect, inset: f32, number: Option<u32>, text: &str, bg: Color32, fg: Color32) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, bg);
    let (y, left) = (rect.center().y, rect.left() + inset);
    if let Some(n) = number {
        p.text(pos2(left + GUTTER, y), Align2::RIGHT_CENTER, n.to_string(), mono(), color::faint());
    }
    p.text(pos2(left + TEXT_X, y), Align2::LEFT_CENTER, text, mono(), fg);
}

/// The accent bar marking the current chunk.
pub(super) fn current_bar(ui: &Ui, rect: Rect) {
    let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, color::accent());
}

/// Width for rows of `columns` columns, each with `inset`: the panel, or wider when a line
/// needs it.
pub(super) fn content_width(ui: &Ui, widest: usize, columns: f32, inset: f32) -> f32 {
    let char_w = ui.fonts(|f| f.glyph_width(&mono(), 'M'));
    ui.available_width().max(columns * (inset + TEXT_X + widest as f32 * char_w + 20.0))
}

/// Display width of the longest line, in characters.
pub(super) fn widest(segs: &[Segment]) -> usize {
    let w = |lines: &[String]| {
        let width = |l: &String| l.trim_end_matches(['\n', '\r']).chars().map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>();
        lines.iter().map(width).max().unwrap_or(0)
    };
    segs.iter()
        .map(|s| match s {
            Segment::Clean(l) => w(l),
            Segment::Conflict(c) => w(&c.ours).max(w(&c.theirs)),
        })
        .max()
        .unwrap_or(0)
}

pub(super) fn panes(
    ui: &mut Ui,
    view: &ConflictView,
    file: &ConflictFile,
    segs: &[Segment],
    scroll_to: Option<usize>,
    h: f32,
    msgs: &mut Vec<Msg>,
) {
    let (rows, firsts) = pane_rows(segs);
    let chunks: Vec<&Chunk> = file.chunks().collect();
    let half_view = (ui.available_width() / 2.0).round();
    // Column headers: the side, its name and Take all.
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, color::toolbar());
    for (side, title, name, r) in [
        (Side::Ours, "Ours", &file.labels.ours, Rect::from_min_max(strip.min, pos2(strip.left() + half_view, strip.bottom()))),
        (Side::Theirs, "Theirs", &file.labels.theirs, Rect::from_min_max(pos2(strip.left() + half_view, strip.top()), strip.max)),
    ] {
        let builder = UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center));
        ui.allocate_new_ui(builder, |ui| {
            ui.label(RichText::new(title).strong().color(color::text()));
            ui.label(RichText::new(name).color(color::weak()));
            if file.no_ancestor {
                ui.label(RichText::new("no common ancestor").small().color(color::faint()));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Take all").on_hover_text(format!("Take {} for every conflict", title.to_lowercase())).clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictTakeAll(side)));
                }
            });
        });
    }
    let width = content_width(ui, widest(segs), 2.0, CHECK_W);
    let half = (width / 2.0).round();
    let mut area = egui::ScrollArea::both().id_salt("conflict-panes").auto_shrink([false, false]).max_height(h - 26.0);
    if let Some(chunk) = scroll_to {
        area = area.vertical_scroll_offset(scroll_target(&firsts, chunk) as f32 * ROW_H);
    }
    area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
        for i in range {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
            let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
            let right = Rect::from_min_max(pos2(left.right(), rect.top()), rect.max);
            match rows[i] {
                PaneRow::Clean { seg, line, ours_no, theirs_no } => {
                    let Segment::Clean(lines) = &segs[seg] else { continue };
                    let text = display(&lines[line]);
                    cell(ui, left, CHECK_W, Some(ours_no), &text, Color32::TRANSPARENT, color::text());
                    cell(ui, right, CHECK_W, Some(theirs_no), &text, Color32::TRANSPARENT, color::text());
                }
                PaneRow::Header(k) => {
                    for (side, r) in [(Side::Ours, left), (Side::Theirs, right)] {
                        chunk_header(ui, r, view, k, side, msgs);
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
                                cell(ui, r, CHECK_W, Some(n), &display(&lines[row]), bg, color::text());
                                line_check(ui, r, view, c, (chunk, side, row), msgs);
                            }
                            None if lines.is_empty() && row == 0 => {
                                let bg = tint(side).gamma_multiply(0.5);
                                cell(ui, r, CHECK_W, None, "(no lines on this side)", bg, color::faint())
                            }
                            None => cell(ui, r, CHECK_W, None, "", color::panel(), color::text()),
                        }
                    }
                    if resp.clicked() {
                        msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(chunk)));
                    }
                }
            }
            ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
        }
    });
}

/// The checkbox at the left of a chunk line's cell `r`: takes or leaves out line `line` of
/// `side` in chunk `k`. Off once the chunk's text was typed in the output.
fn line_check(ui: &mut Ui, r: Rect, view: &ConflictView, c: &Chunk, (k, side, line): (usize, Side, usize), msgs: &mut Vec<Msg>) {
    let pick = &view.picks[k];
    let mut taken = pick.takes_line(c, side, line);
    let rect = Rect::from_min_max(pos2(r.left() + 2.0, r.top()), pos2(r.left() + CHECK_W, r.bottom()));
    let mut builder = UiBuilder::new().max_rect(rect).layout(Layout::centered_and_justified(Direction::TopDown));
    if pick.is_edited() {
        builder = builder.disabled();
    }
    // A child that doesn't move the rows' cursor; added after the row, so it gets the click.
    let mut child = ui.new_child(builder);
    let tip = if taken { "Leave this line out" } else { "Take this line" };
    let resp = child.add(egui::Checkbox::without_text(&mut taken)).on_hover_text(tip).on_disabled_hover_text(EDITED_TIP);
    if resp.changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggleLine { chunk: k, side, line }));
    }
}

/// A chunk's header in one column: "Conflict N" (with a faint tag when its pick was made
/// automatically or its text typed in the output) and a Take checkbox for that side (ticked
/// when every line of it is taken, mixed when some are). A chunk edited in the output has
/// Discard edit in the theirs column instead.
fn chunk_header(ui: &mut Ui, r: Rect, view: &ConflictView, k: usize, side: Side, msgs: &mut Vec<Msg>) {
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    p.rect_filled(r, 0.0, color::hunk_bg());
    let title = p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, format!("Conflict {}", k + 1), mono(), color::hunk_fg());
    if k == view.current {
        current_bar(ui, r);
    }
    let (Some(pick), Some(c)) = (view.picks.get(k), view.chunk(k)) else { return };
    let edited = pick.is_edited();
    // Kept in view when the panes scroll sideways.
    let x = r.right().min(ui.clip_rect().right()) - 8.0;
    let check = Rect::from_min_max(pos2(x - if edited { 90.0 } else { 64.0 }, r.top()), pos2(x, r.bottom()));
    let tag = match view.auto.get(k).copied().flatten() {
        _ if edited => Some("edited in the output".to_owned()),
        Some(why) => Some(format!("picked automatically · {}", why.label())),
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
            let b = ui.put(check, egui::Button::new(RichText::new("Discard edit").small()));
            if b.on_hover_text("Drop the text typed in the output and bring back the picks").clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictDiscardEdit(k)));
            }
        }
        return;
    }
    let mut taken = pick.takes_side(c, side);
    let check_box = egui::Checkbox::new(&mut taken, "Take").indeterminate(pick.partly_takes_side(c, side));
    if ui.put(check, check_box).changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggle { chunk: k, side }));
    }
}
