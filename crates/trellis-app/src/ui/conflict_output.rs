//! The conflict view's output, as it will be saved: painted rows tinted by where each line came
//! from (a placeholder for each conflict still to pick), or, with Edit on, the whole file as
//! text to change by hand, conflicts not picked yet shown as markers.

use super::conflict_panes::{cell, content_width, current_bar, display, tint, widest};
use super::diff::{ROW_H, mono};
use super::theme::color;
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict::{ConflictView, OutputEdit};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke, Ui, UiBuilder, vec2};
use trellis_core::conflict_pick::ChunkPick;
use trellis_core::conflict_rows::{OutRow, output_rows, scroll_target};
use trellis_core::repo::{Chunk, ConflictContent, ConflictFile, Segment};

/// The Output strip (with the Edit / Done toggle), the note after an undone edit, and the
/// output itself.
pub(super) fn output(ui: &mut Ui, view: &mut ConflictView, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let (n, resolved) = (view.chunk_count(), view.chunk_count() - view.unresolved());
    let ConflictView { file, picks, edit, edit_note, current, saving, .. } = view;
    let Some(f) = file.as_ref() else { return };
    let ConflictContent::Text(segs) = &f.content else { return };
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, color::toolbar());
    ui.painter().hline(strip.x_range(), strip.top(), Stroke::new(1.0_f32, color::border()));
    let builder = UiBuilder::new().max_rect(strip.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center));
    ui.allocate_new_ui(builder, |ui| {
        ui.label(RichText::new("Output").strong().color(color::text()));
        ui.label(RichText::new(format!("{resolved} of {n} conflicts resolved")).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let on = edit.is_some();
            let tip = if on { "Back to the picked lines" } else { "Edit the conflicts' text by hand" };
            if ui.selectable_label(on, if on { "Done" } else { "Edit" }).on_hover_text(tip).clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictEditOutput(!on)));
            }
        });
    });
    if let Some(note) = edit_note {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(note.as_str()).color(color::amber()));
        });
    }
    match edit {
        Some(e) => editor(ui, e, *saving, scroll_to, msgs),
        None => painted(ui, f, segs, picks, *current, scroll_to, msgs),
    }
}

/// The painted output: clean lines, taken lines tinted by side in pick order, typed lines in
/// the accent tint, and a placeholder for each chunk still to pick.
fn painted(
    ui: &mut Ui,
    file: &ConflictFile,
    segs: &[Segment],
    picks: &[ChunkPick],
    current: usize,
    scroll_to: Option<usize>,
    msgs: &mut Vec<Msg>,
) {
    let (rows, firsts) = output_rows(segs, picks);
    let chunks: Vec<&Chunk> = file.chunks().collect();
    // Output line numbers (placeholders have none).
    let mut next = 1u32;
    let numbers: Vec<Option<u32>> = rows
        .iter()
        .map(|r| {
            (!matches!(r, OutRow::Unresolved(_))).then(|| {
                next += 1;
                next - 1
            })
        })
        .collect();
    let width = content_width(ui, widest(segs), 1.0, 0.0);
    let mut area = egui::ScrollArea::both().id_salt("conflict-output").auto_shrink([false, false]);
    if let Some(chunk) = scroll_to {
        area = area.vertical_scroll_offset(scroll_target(&firsts, chunk) as f32 * ROW_H);
    }
    area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
        for i in range {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
            match rows[i] {
                OutRow::Clean { seg, line } => {
                    let Segment::Clean(lines) = &segs[seg] else { continue };
                    cell(ui, rect, 0.0, numbers[i], &display(&lines[line]), Color32::TRANSPARENT, color::text());
                }
                OutRow::Taken { chunk, side, line } => {
                    cell(ui, rect, 0.0, numbers[i], &display(&chunks[chunk].side(side)[line]), tint(side), color::text());
                }
                OutRow::Edited { chunk, line } => {
                    let ChunkPick::Edited { lines, .. } = &picks[chunk] else { continue };
                    cell(ui, rect, 0.0, numbers[i], &display(&lines[line]), color::green().gamma_multiply(0.18), color::text());
                    if chunk == current && firsts.get(chunk) == Some(&i) {
                        current_bar(ui, rect);
                    }
                    if resp.clicked() {
                        msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(chunk)));
                    }
                }
                OutRow::Unresolved(k) => {
                    let text = format!("Conflict {}: pick a side above", k + 1);
                    cell(ui, rect, 0.0, None, &text, color::amber().gamma_multiply(0.18), color::amber());
                    if k == current {
                        current_bar(ui, rect);
                    }
                    if resp.clicked() {
                        msgs.push(Msg::Conflict(ConflictMsg::ConflictSelect(k)));
                    }
                }
            }
        }
    });
}

/// The whole file as text (conflicts not picked yet as markers). A change is reported as
/// `ConflictOutputEdited`; the update keeps it only inside one conflict. Read-only while saving.
fn editor(ui: &mut Ui, e: &mut OutputEdit, saving: bool, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let mut area = egui::ScrollArea::both().id_salt("conflict-output-edit").auto_shrink([false, false]);
    if let Some(range) = scroll_to.and_then(|k| e.rendered.chunks.get(k)) {
        let row_h = ui.fonts(|f| f.row_height(&mono()));
        let line = e.rendered.text[..range.start].matches('\n').count();
        area = area.vertical_scroll_offset(line.saturating_sub(3) as f32 * row_h);
    }
    area.show(ui, |ui| {
        let text = egui::TextEdit::multiline(&mut e.buffer)
            .code_editor()
            .font(mono())
            .lock_focus(true)
            .desired_width(f32::INFINITY)
            .min_size(vec2(0.0, ui.available_height()));
        if ui.add_enabled(!saving, text).changed() {
            msgs.push(Msg::Conflict(ConflictMsg::ConflictOutputEdited));
        }
    });
}
