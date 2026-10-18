//! The history's File view: the whole file as of the selected commit, in the editor font with
//! optional line numbers and syntax colors, long lines clipped. The text comes from
//! `FileHistoryView::text` (read by the runner); this only draws it.

use super::diff::{DiffStyle, paint_line};
use super::theme::color;
use crate::state::AppState;
use crate::state::history::{FileTextData, TextState};
use eframe::egui::{self, Align2, Rect, RichText, Sense, Ui, pos2, vec2};

pub(super) fn show(ui: &mut Ui, st: &AppState) {
    let Some(view) = &st.history else { return };
    let Some((oid, path, status)) = view.shown() else { return };
    let text = view.text.as_ref().filter(|t| t.oid == oid && t.path == path);
    match text.map(|t| &t.state) {
        _ if status == 'D' => note(ui, "This commit deletes the file", false),
        None | Some(TextState::Loading) => note(ui, "Loading…", false),
        Some(TextState::Failed(e)) => note(ui, e, true),
        Some(TextState::Ready(data)) if data.lines.is_empty() => note(ui, "This file is empty", false),
        Some(TextState::Ready(data)) => lines(ui, st, data, (oid, path)),
    }
}

/// A line of weak text below the commit block; `wrap` for messages that can run long.
fn note(ui: &mut Ui, text: &str, wrap: bool) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let label = egui::Label::new(RichText::new(text).color(color::weak()));
        ui.add(if wrap { label.wrap() } else { label });
    });
}

/// The file's lines, only those in view laid out. Each commit and file scrolls on its own, from
/// the top.
fn lines(ui: &mut Ui, st: &AppState, data: &FileTextData, (oid, path): (git2::Oid, &str)) {
    let style = DiffStyle::new(&st.settings);
    let n = data.lines.len();
    let digit_w = ui.fonts(|f| f.glyph_width(&style.font, '0'));
    let num_w = if style.numbers { digit_w * n.to_string().len() as f32 } else { 0.0 };
    let syntax = st.settings.syntax_highlighting;
    let bg = ui.available_rect_before_wrap();
    ui.painter().rect_filled(bg, 0.0, color::bg());
    ui.spacing_mut().item_spacing.y = 0.0;
    let area = egui::ScrollArea::vertical().id_salt(("history-file", oid, path)).auto_shrink([false, false]);
    area.show_rows(ui, style.row_h, n, |ui, range| {
        let w = ui.available_width();
        for i in range {
            let (r, _) = ui.allocate_exact_size(vec2(w, style.row_h), Sense::hover());
            let y = r.center().y;
            let num_right = r.left() + 8.0 + num_w;
            if style.numbers {
                ui.painter().text(pos2(num_right, y), Align2::RIGHT_CENTER, (i + 1).to_string(), style.font.clone(), color::faint());
            }
            let code_left = if style.numbers { num_right + 12.0 } else { r.left() + 8.0 };
            let clip = Rect::from_min_max(pos2(code_left, r.top()), r.max);
            let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
            let spans = if syntax { data.spans(i) } else { &[] };
            paint_line(&painter, pos2(code_left, y), &data.lines[i], spans, None, &style.font, color::text(), style.tab, None);
        }
    });
}
