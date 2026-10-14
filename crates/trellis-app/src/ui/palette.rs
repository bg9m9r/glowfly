//! The command palette (Ctrl+P): a search field over the commands available now, ranked by a
//! fuzzy match. The list comes from `state::palette::shown`; this file only draws it and turns
//! keys and clicks into messages.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::palette::{self, Shown};
use eframe::egui::{self, Align2, FontId, Id, Key, Modifiers, Rect, Response, Sense, Ui, pos2, text::LayoutJob, vec2};

const WIDTH: f32 = 560.0;
const ROW_H: f32 = 28.0;
const LIST_MAX_H: f32 = 360.0;
const LABEL_SIZE: f32 = 13.5;

/// Ctrl+P or Ctrl+Shift+P (Cmd on macOS) opens or closes the palette. Works while a text field
/// has focus; does nothing under Preferences, a dialog or a credential prompt.
pub fn shortcut(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() || st.modal.is_some() || st.askpass.showing() {
        return;
    }
    ctx.input_mut(|i| {
        // Shift first: consume_key ignores extra Shift, so Ctrl+P would also match Ctrl+Shift+P.
        if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::P) || i.consume_key(Modifiers::COMMAND, Key::P) {
            msgs.push(Msg::TogglePalette);
        }
    });
}

/// Draws the open palette. A credential prompt above it keeps the keyboard.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.palette.is_none() {
        return;
    }
    let prompting = st.askpass.showing();
    let id = Id::new("trellis-palette");
    let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, vec2(0.0, 80.0));
    let resp = egui::Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_width(WIDTH);
        let mut run_selected = false;
        if !prompting {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    msgs.push(Msg::PaletteMove(1));
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    msgs.push(Msg::PaletteMove(-1));
                }
                run_selected = i.consume_key(Modifiers::NONE, Key::Enter);
            });
        }
        let Some(p) = st.palette.as_mut() else { return };
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            widgets::paint_icon(ui.painter(), rect.center(), Icon::Search, color::weak());
            let r = ui.add(
                egui::TextEdit::singleline(&mut p.query)
                    .id(Id::new("trellis-palette-query"))
                    .hint_text("Type a command or branch name")
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(15.0)),
            );
            if !prompting {
                r.request_focus();
            }
            if r.changed() {
                p.selected = 0;
            }
        });
        ui.add_space(6.0);
        let query = p.query.clone();
        let list = palette::shown(st, &query);
        let Some(p) = st.palette.as_mut() else { return };
        let sel = p.selected_in(list.len());
        if run_selected && let Some(s) = sel {
            msgs.push(Msg::RunPaletteCommand(list[s].entry.command.clone()));
        }
        if list.is_empty() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("No matching commands").color(color::weak()));
            ui.add_space(4.0);
        } else {
            egui::ScrollArea::vertical().id_salt("trellis-palette-list").max_height(LIST_MAX_H).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, shown) in list.iter().enumerate() {
                    let selected = sel == Some(i);
                    let r = row(ui, shown, selected);
                    if r.clicked() {
                        msgs.push(Msg::RunPaletteCommand(shown.entry.command.clone()));
                    }
                    if selected && p.scroll_to_selected {
                        r.scroll_to_me(None);
                        p.scroll_to_selected = false;
                    }
                }
            });
        }
        ui.add_space(6.0);
        let footer = "Up/Down to choose, Enter to run, Esc to close";
        ui.label(egui::RichText::new(footer).size(12.0).color(color::weak()));
    });
    if resp.should_close() && !prompting {
        msgs.push(Msg::ClosePalette);
    }
}

/// One entry: the label with its matched characters in the accent colour, and the detail in
/// dim text at the right.
fn row(ui: &mut Ui, shown: &Shown, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let label = &shown.entry.label;
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 3.0, color::selected());
    } else if resp.hovered() {
        p.rect_filled(rect, 3.0, color::hover());
    }
    let mut right_w = 0.0;
    if let Some(detail) = &shown.entry.detail {
        let g = p.layout_no_wrap(detail.clone(), FontId::proportional(12.0), color::weak());
        right_w = g.size().x + 18.0;
        p.galley(pos2(rect.right() - g.size().x - 10.0, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    let g = ui.fonts(|f| f.layout_job(label_job(label, &shown.matched)));
    let text_rect = Rect::from_min_max(pos2(rect.left() + 10.0, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    p.with_clip_rect(text_rect).galley(pos2(text_rect.left(), rect.center().y - g.size().y / 2.0), g, color::text());
    resp
}

/// The label as runs of plain and matched characters.
fn label_job(label: &str, matched: &[usize]) -> LayoutJob {
    let mut job = LayoutJob::default();
    let format = |hit: bool| egui::TextFormat {
        font_id: FontId::proportional(LABEL_SIZE),
        color: if hit { color::accent() } else { color::text() },
        ..Default::default()
    };
    let mut run = String::new();
    let mut run_hit = false;
    for (i, c) in label.chars().enumerate() {
        let hit = matched.contains(&i);
        if hit != run_hit && !run.is_empty() {
            job.append(&std::mem::take(&mut run), 0.0, format(run_hit));
        }
        run_hit = hit;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, format(run_hit));
    }
    job
}
