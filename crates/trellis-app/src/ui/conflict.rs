//! The conflict view: ours and theirs side by side with a pick per conflict chunk, and below
//! them the output as it will be saved. Binary, deleted and other special files offer a whole
//! version instead.

use super::diff::{ROW_H, mono};
use super::theme::color;
use super::widgets::{Icon, icon_button};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use eframe::egui::{self, Align, Align2, Color32, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use trellis_core::conflict_rows::{OutRow, PaneRow, output_rows, pane_rows, scroll_target};
use trellis_core::repo::{Chunk, ConflictContent, ConflictFile, Operation, Segment, Side, SideLabels};

/// Line numbers end here; text starts at `TEXT_X`.
const GUTTER: f32 = 40.0;
const TEXT_X: f32 = 50.0;

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.conflict_scroll_to.take();
    let Some(view) = &st.conflict else { return };
    ui.add_space(6.0);
    header_bar(ui, view, msgs);
    ui.separator();
    let Some(file) = &view.file else {
        ui.label(RichText::new("Loading…").color(color::weak()));
        return;
    };
    match &file.content {
        ConflictContent::Text(segs) => {
            ui.spacing_mut().item_spacing.y = 0.0;
            let top_h = (ui.available_height() * 0.55).floor();
            let width = ui.available_width();
            ui.allocate_ui_with_layout(vec2(width, top_h), Layout::top_down(Align::Min), |ui| {
                panes(ui, view, file, segs, scroll_to, top_h, msgs);
            });
            output(ui, view, segs, scroll_to, msgs);
        }
        other => fallback(ui, view, file, other, msgs),
    }
}

fn operation_text(l: &SideLabels) -> String {
    let (ours, theirs) = (&l.ours, &l.theirs);
    match l.operation {
        Operation::Merge => format!("Merging {theirs} into {ours}"),
        Operation::Rebase => format!("Rebasing {theirs} onto {ours}"),
        Operation::CherryPick => format!("Cherry-picking {theirs} onto {ours}"),
        Operation::Revert => format!("Reverting {theirs} on {ours}"),
        Operation::Other => format!("Applying changes onto {ours}"),
    }
}

/// Path and operation on the left; on the right (from the edge inwards) Close, Mark resolved,
/// Save, Reset and the conflict counter with previous/next.
fn header_bar(ui: &mut Ui, view: &ConflictView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(&view.path).strong().size(15.0).color(color::text()));
        if let Some(f) = &view.file {
            ui.label(RichText::new(operation_text(&f.labels)).color(color::weak()));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::CloseConflict);
            }
            if ui.button("Mark resolved").on_hover_text("Stage the working file as it is now").clicked() {
                msgs.push(Msg::StageFile(view.path.clone()));
            }
            if !view.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_))) {
                return;
            }
            let can = view.can_save();
            let save = egui::Button::new(RichText::new("Save").strong().color(if can { Color32::WHITE } else { color::faint() }))
                .fill(if can { color::primary() } else { color::toolbar() });
            let why = if view.saving {
                "Saving…".to_owned()
            } else {
                format!("Pick a side for every conflict first ({} left)", view.unresolved())
            };
            if ui
                .add_enabled(can, save)
                .on_hover_text("Write the output to the file and mark it resolved")
                .on_disabled_hover_text(why)
                .clicked()
            {
                msgs.push(Msg::SaveConflict);
            }
            if ui.button("Reset").on_hover_text("Clear all picks").clicked() {
                msgs.push(Msg::ConflictReset);
            }
            ui.add_space(10.0);
            let n = view.chunk_count();
            if n == 0 {
                ui.label(RichText::new("No conflicting lines").color(color::weak()));
                return;
            }
            // Right to left: next first. Arrows are painted (the bundled fonts lack them).
            let next = ui.add_enabled_ui(view.current + 1 < n, |ui| icon_button(ui, Icon::NextChange, "Next conflict (F7)")).inner;
            if next.clicked() {
                msgs.push(Msg::ConflictStep(1));
            }
            let counter = if n == 1 { "1 conflict".to_owned() } else { format!("conflict {} of {n}", view.current + 1) };
            ui.label(RichText::new(counter).color(color::text()));
            let prev = ui.add_enabled_ui(view.current > 0, |ui| icon_button(ui, Icon::PrevChange, "Previous conflict (Shift+F7)")).inner;
            if prev.clicked() {
                msgs.push(Msg::ConflictStep(-1));
            }
        });
    });
}

/// Trims the terminator and shows tabs as 4 spaces (the file is written unchanged).
fn display(line: &str) -> String {
    line.trim_end_matches('\n').trim_end_matches('\r').replace('\t', "    ")
}

fn tint(side: Side) -> Color32 {
    match side {
        Side::Ours => color::blue().gamma_multiply(0.18),
        Side::Theirs => color::purple().gamma_multiply(0.18),
    }
}

/// One cell: background, line number and text, clipped to its column.
fn cell(ui: &Ui, rect: Rect, number: Option<u32>, text: &str, bg: Color32, fg: Color32) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, bg);
    let y = rect.center().y;
    if let Some(n) = number {
        p.text(pos2(rect.left() + GUTTER, y), Align2::RIGHT_CENTER, n.to_string(), mono(), color::faint());
    }
    p.text(pos2(rect.left() + TEXT_X, y), Align2::LEFT_CENTER, text, mono(), fg);
}

/// The accent bar marking the current chunk.
fn current_bar(ui: &Ui, rect: Rect) {
    let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, color::accent());
}

/// Width for rows: the panel, or wider when a line needs it.
fn content_width(ui: &Ui, widest: usize, columns: f32) -> f32 {
    let char_w = ui.fonts(|f| f.glyph_width(&mono(), 'M'));
    ui.available_width().max(columns * (TEXT_X + widest as f32 * char_w + 20.0))
}

/// Display width of the longest line, in characters.
fn widest(segs: &[Segment]) -> usize {
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

fn panes(ui: &mut Ui, view: &ConflictView, file: &ConflictFile, segs: &[Segment], scroll_to: Option<usize>, h: f32, msgs: &mut Vec<Msg>) {
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
                    msgs.push(Msg::ConflictTakeAll(side));
                }
            });
        });
    }
    let width = content_width(ui, widest(segs), 2.0);
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
                    cell(ui, left, Some(ours_no), &text, Color32::TRANSPARENT, color::text());
                    cell(ui, right, Some(theirs_no), &text, Color32::TRANSPARENT, color::text());
                }
                PaneRow::Header(k) => {
                    for (side, r) in [(Side::Ours, left), (Side::Theirs, right)] {
                        chunk_header(ui, r, view, k, side, msgs);
                    }
                    if resp.clicked() {
                        msgs.push(Msg::ConflictSelect(k));
                    }
                }
                PaneRow::Lines { chunk, row, ours, theirs } => {
                    for (side, r, number) in [(Side::Ours, left, ours), (Side::Theirs, right, theirs)] {
                        let lines = chunks[chunk].side(side);
                        match number {
                            Some(n) => cell(ui, r, Some(n), &display(&lines[row]), tint(side), color::text()),
                            None if lines.is_empty() && row == 0 => {
                                cell(ui, r, None, "(no lines on this side)", tint(side).gamma_multiply(0.5), color::faint())
                            }
                            None => cell(ui, r, None, "", color::panel(), color::text()),
                        }
                    }
                    if resp.clicked() {
                        msgs.push(Msg::ConflictSelect(chunk));
                    }
                }
            }
            ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
        }
    });
}

/// A chunk's header in one column: "Conflict N" and a Take checkbox for that side.
fn chunk_header(ui: &mut Ui, r: Rect, view: &ConflictView, k: usize, side: Side, msgs: &mut Vec<Msg>) {
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    p.rect_filled(r, 0.0, color::hunk_bg());
    p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, format!("Conflict {}", k + 1), mono(), color::hunk_fg());
    if k == view.current {
        current_bar(ui, r);
    }
    // Kept in view when the panes scroll sideways.
    let x = r.right().min(ui.clip_rect().right()) - 8.0;
    let check = Rect::from_min_max(pos2(x - 64.0, r.top()), pos2(x, r.bottom()));
    let mut taken = view.choices.get(k).is_some_and(|c| c.takes(side));
    if ui.put(check, egui::Checkbox::new(&mut taken, "Take")).changed() {
        msgs.push(Msg::ConflictToggle { chunk: k, side });
    }
}

/// The file as it will be saved, with a placeholder for each chunk still to pick.
fn output(ui: &mut Ui, view: &ConflictView, segs: &[Segment], scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let (rows, firsts) = output_rows(segs, &view.choices);
    let chunks: Vec<&Chunk> = view.file.iter().flat_map(|f| f.chunks()).collect();
    let n = view.chunk_count();
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, color::toolbar());
    ui.painter().hline(strip.x_range(), strip.top(), Stroke::new(1.0_f32, color::border()));
    let builder = UiBuilder::new().max_rect(strip.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center));
    ui.allocate_new_ui(builder, |ui| {
        ui.label(RichText::new("Output").strong().color(color::text()));
        ui.label(RichText::new(format!("{} of {n} conflicts resolved", n - view.unresolved())).color(color::weak()));
    });
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
    let width = content_width(ui, widest(segs), 1.0);
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
                    cell(ui, rect, numbers[i], &display(&lines[line]), Color32::TRANSPARENT, color::text());
                }
                OutRow::Taken { chunk, side, line } => {
                    cell(ui, rect, numbers[i], &display(&chunks[chunk].side(side)[line]), tint(side), color::text());
                }
                OutRow::Unresolved(k) => {
                    let text = format!("Conflict {}: pick a side above", k + 1);
                    cell(ui, rect, None, &text, color::amber().gamma_multiply(0.18), color::amber());
                    if k == view.current {
                        current_bar(ui, rect);
                    }
                    if resp.clicked() {
                        msgs.push(Msg::ConflictSelect(k));
                    }
                }
            }
        }
    });
}

/// Files with no chunk view: the reason, and buttons to take a whole version.
fn fallback(ui: &mut Ui, view: &ConflictView, file: &ConflictFile, content: &ConflictContent, msgs: &mut Vec<Msg>) {
    let labels = &file.labels;
    let message = match content {
        ConflictContent::Binary => "Binary file: pick a whole version.".to_owned(),
        ConflictContent::DeletedBy(Side::Theirs) => format!("Deleted on the theirs side ({})", labels.theirs),
        ConflictContent::DeletedBy(Side::Ours) => format!("Deleted on the ours side ({})", labels.ours),
        ConflictContent::Unsupported(reason) => format!("This file can't be merged here: {reason}."),
        ConflictContent::Text(_) => return,
    };
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(message).size(14.0).color(color::text()));
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let path = &view.path;
        for (side, word, name) in [(Side::Ours, "ours", &labels.ours), (Side::Theirs, "theirs", &labels.theirs)] {
            let deleted = *content == ConflictContent::DeletedBy(side);
            let (label, body) = if deleted {
                (format!("Delete file ({name})"), format!("Delete {path} and mark it resolved? Changes in the working file are lost."))
            } else {
                (
                    format!("Use {word} ({name})"),
                    format!("Replace {path} with the {word} version and mark it resolved? Changes in the working file are lost."),
                )
            };
            if ui.add_enabled(!view.saving, egui::Button::new(&label)).clicked() {
                let resolve = Msg::ResolveWithSide { path: path.clone(), side };
                msgs.push(Msg::confirm("Resolve conflict", body, if deleted { "Delete" } else { "Replace" }, resolve));
            }
        }
        if ui.button("Mark resolved").on_hover_text("Stage the working file as it is now").clicked() {
            msgs.push(Msg::StageFile(path.clone()));
        }
    });
}
