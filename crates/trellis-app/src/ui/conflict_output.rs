//! The conflict view's output, as it will be saved: painted rows tinted by where each line came
//! from (a placeholder for each conflict still to pick, typed lines in green, long clean runs
//! folded), or, with Edit on, the whole file as text to change by hand, conflicts not picked
//! yet shown as markers.

use super::conflict_cells::{cell, conflict_wrap, content_width, current_bar, display, offsets, tint, visual_lines, widest};
use super::conflict_fold::fold_row;
use super::diff::wrap::RowMetrics;
use super::diff::{ROW_H, mono};
use super::theme::color;
use crate::msg::{ConflictMsg, DiffMsg, Msg};
use crate::state::conflict::{ConflictView, OutputEdit};
use crate::ui::theme::controls;
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke, Ui, UiBuilder, vec2};
use trellis_core::conflict_pick::ChunkPick;
use trellis_core::conflict_rows::{OutRow, output_rows, scroll_target};
use trellis_core::repo::{Chunk, Segment};

/// The tint of text typed in the output.
fn typed() -> Color32 {
    color::green().gamma_multiply(0.18)
}

/// The Output strip (Edit / Done, Wrap lines, and Discard edits outside conflicts while any
/// clean run is edited), the note after an undone edit, and the output itself. `wrap` is the
/// Wrap long lines setting.
pub(super) fn output(ui: &mut Ui, view: &mut ConflictView, scroll_to: Option<usize>, wrap: bool, msgs: &mut Vec<Msg>) {
    let (n, resolved) = (view.chunk_count(), view.chunk_count() - view.unresolved());
    if view.segments().is_none() {
        return;
    }
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, color::toolbar());
    ui.painter().hline(strip.x_range(), strip.top(), Stroke::new(1.0_f32, color::border()));
    let builder = UiBuilder::new().max_rect(strip.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center));
    ui.allocate_new_ui(builder, |ui| {
        ui.label(RichText::new("Output").strong().color(color::text()));
        ui.label(RichText::new(format!("{resolved} of {n} conflicts resolved")).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let on = view.edit.is_some();
            let tip = if on { "Back to the picked lines" } else { "Edit the output by hand" };
            if ui.selectable_label(on, if on { "Done" } else { "Edit" }).on_hover_text(tip).clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictEditOutput(!on)));
            }
            if ui.selectable_label(wrap, "Wrap lines").on_hover_text("Wrap long lines instead of scrolling sideways").clicked() {
                msgs.push(Msg::Diff(DiffMsg::SetWordWrap(!wrap)));
            }
            if !view.cleans.is_empty()
                && ui
                    .small_button("Discard edits outside conflicts")
                    .on_hover_text("Bring back the file's own lines between the conflicts")
                    .clicked()
            {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictDiscardCleanEdits));
            }
        });
    });
    if let Some(note) = &view.edit_note {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(note.as_str()).color(color::amber()));
        });
    }
    let saving = view.saving;
    match view.edit.as_mut() {
        Some(e) => editor(ui, e, saving, scroll_to, wrap, msgs),
        None => painted(ui, view, scroll_to, wrap, msgs),
    }
}

/// The painted output: clean lines (long runs folded), taken lines tinted by side in pick
/// order, typed lines (in a conflict or between them) in green, and a placeholder for each
/// chunk still to pick.
fn painted(ui: &mut Ui, view: &ConflictView, scroll_to: Option<usize>, wrap_on: bool, msgs: &mut Vec<Msg>) {
    let (Some(file), Some(segs)) = (view.file.as_ref(), view.segments()) else { return };
    let (picks, cleans, current) = (&view.picks, &view.cleans, view.current);
    let (rows, firsts) = output_rows(segs, picks, cleans, &view.opened);
    let chunks: Vec<&Chunk> = file.chunks().collect();
    // Output line numbers: placeholders have none, folds count the lines they hide.
    let mut next = 1u32;
    let numbers: Vec<Option<u32>> = rows
        .iter()
        .map(|r| match r {
            OutRow::Unresolved(_) => None,
            OutRow::Fold(f) => {
                next += f.len() as u32;
                None
            }
            _ => {
                next += 1;
                Some(next - 1)
            }
        })
        .collect();
    let text_of = |r: &OutRow| -> Option<String> {
        let line = match *r {
            OutRow::Clean { seg, line } => match &segs[seg] {
                Segment::Clean(lines) => &lines[line],
                Segment::Conflict(_) => return None,
            },
            OutRow::CleanEdited { seg, line } => &cleans.get(&seg)?[line],
            OutRow::Taken { chunk, side, line } => &chunks[chunk].side(side)[line],
            OutRow::Edited { chunk, line } => match &picks[chunk] {
                ChunkPick::Edited { lines, .. } => &lines[line],
                _ => return None,
            },
            OutRow::Fold(_) | OutRow::Unresolved(_) => return None,
        };
        Some(display(line))
    };
    let wrap = conflict_wrap(ui, wrap_on, 1.0, 0.0);
    let width = content_width(ui, widest(segs), 1.0, 0.0, wrap.is_some());
    let heights;
    let m = match wrap {
        Some(w) => {
            heights = offsets(rows.len(), w, |i| text_of(&rows[i]).map_or(1, |t| visual_lines(&t, wrap)));
            RowMetrics::Varied(&heights)
        }
        None => RowMetrics::Uniform(ROW_H),
    };
    super::diff::wrap::scroll(ui, "conflict-output", rows.len(), m, scroll_to.map(|k| scroll_target(&firsts, k)), |ui, i, h| {
        let width = if wrap.is_some() { ui.available_width() } else { width };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
        let text = text_of(&rows[i]).unwrap_or_default();
        match rows[i] {
            OutRow::Clean { .. } => cell(ui, rect, 0.0, numbers[i], &text, (Color32::TRANSPARENT, color::text()), wrap),
            OutRow::CleanEdited { .. } => cell(ui, rect, 0.0, numbers[i], &text, (typed(), color::text()), wrap),
            OutRow::Fold(f) => fold_row(ui, rect, f, msgs),
            OutRow::Taken { side, .. } => cell(ui, rect, 0.0, numbers[i], &text, (tint(side), color::text()), wrap),
            OutRow::Edited { chunk, .. } => {
                cell(ui, rect, 0.0, numbers[i], &text, (typed(), color::text()), wrap);
                if chunk == current && firsts.get(chunk) == Some(&i) {
                    current_bar(ui, rect);
                }
                if resp.clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(chunk)));
                }
            }
            OutRow::Unresolved(k) => {
                let text = format!("Conflict {}: pick a side above", k + 1);
                cell(ui, rect, 0.0, None, &text, (color::amber().gamma_multiply(0.18), color::amber()), None);
                if k == current {
                    current_bar(ui, rect);
                }
                if resp.clicked() {
                    msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(k)));
                }
            }
        }
    });
}

/// The whole file as text (conflicts not picked yet as markers), with no folds; wrapped (`wrap`)
/// as wide as the panel. A change is reported as `ConflictOutputEdited`; the update keeps it
/// only inside one conflict or one clean run. Read-only while saving.
fn editor(ui: &mut Ui, e: &mut OutputEdit, saving: bool, scroll_to: Option<usize>, wrap: bool, msgs: &mut Vec<Msg>) {
    let area = if wrap { egui::ScrollArea::vertical() } else { egui::ScrollArea::both() };
    let mut area = area.id_salt("conflict-output-edit").auto_shrink([false, false]);
    if let Some(range) = scroll_to.and_then(|k| e.rendered.chunks.get(k)) {
        let row_h = ui.fonts(|f| f.row_height(&mono()));
        let line = e.rendered.text[..range.start].matches('\n').count();
        area = area.vertical_scroll_offset(line.saturating_sub(3) as f32 * row_h);
    }
    area.show(ui, |ui| {
        let width = if wrap { ui.available_width() } else { f32::INFINITY };
        let text = egui::TextEdit::multiline(&mut e.buffer)
            .code_editor()
            .font(mono())
            .lock_focus(true)
            .desired_width(width)
            .min_size(vec2(0.0, ui.available_height()));
        if ui.add_enabled(!saving, controls::Field(text)).changed() {
            msgs.push(Msg::Conflict(ConflictMsg::ConflictOutputEdited));
        }
    });
}
