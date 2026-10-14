//! The command palette (Ctrl+P): a search field over the commands available now, ranked by a
//! fuzzy match, or over a two-step command's items (tracked files for File history). The lists
//! come from `state::palette`; this file only draws them and turns keys and clicks into messages.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::palette::{self, Items, Mode, Palette};
use eframe::egui::{self, Align2, Color32, FontId, Id, Key, Modifiers, Rect, Response, RichText, Sense, Ui, pos2, text::LayoutJob, vec2};

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
    let Some(p) = st.palette.as_ref() else { return };
    let picking = matches!(p.mode, Mode::Pick(_));
    let empty = p.query.is_empty();
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
                // In a sub-list, Backspace on an empty field and Esc go back to the commands.
                // Taken before the field draws, and before the modal would close on Esc.
                let back = |i: &mut egui::InputState| {
                    (empty && i.consume_key(Modifiers::NONE, Key::Backspace)) | i.consume_key(Modifiers::NONE, Key::Escape)
                };
                if picking && back(i) {
                    msgs.push(Msg::PaletteBack);
                }
            });
        }
        let Some(p) = st.palette.as_mut() else { return };
        search_row(ui, p, prompting, msgs);
        ui.add_space(6.0);
        let footer = if picking {
            pick_list(ui, p, msgs, run_selected)
        } else {
            commands_list(ui, st, msgs, run_selected);
            "Up/Down to choose, Enter to run, Esc to close"
        };
        ui.add_space(6.0);
        ui.label(RichText::new(footer).size(12.0).color(color::weak()));
    });
    if resp.should_close() && !prompting {
        msgs.push(Msg::ClosePalette);
    }
}

/// The search field, after a search icon, or in a sub-list after a back arrow and its title.
fn search_row(ui: &mut Ui, p: &mut Palette, prompting: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let hint = match &p.mode {
            Mode::Commands => {
                let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Search, color::weak());
                "Type a command or branch name"
            }
            Mode::Pick(pk) => {
                let (rect, r) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                let col = if r.hovered() { color::text() } else { color::weak() };
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Back, col);
                if r.on_hover_text("Back to commands (Backspace)").clicked() {
                    msgs.push(Msg::PaletteBack);
                }
                ui.label(RichText::new(pk.kind.title()).size(12.5).color(color::weak()));
                pk.kind.prompt()
            }
        };
        let r = ui.add(
            egui::TextEdit::singleline(&mut p.query)
                .id(Id::new("trellis-palette-query"))
                .hint_text(hint)
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
}

/// The commands matching the query.
fn commands_list(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>, run_selected: bool) {
    let Some(query) = st.palette.as_ref().map(|p| p.query.clone()) else { return };
    let list = palette::shown(st, &query);
    let Some(p) = st.palette.as_mut() else { return };
    let sel = p.selected_in(list.len());
    if run_selected && let Some(s) = sel {
        msgs.push(Msg::RunPaletteCommand(list[s].entry.command.clone()));
    }
    if list.is_empty() {
        note(ui, "No matching commands", color::weak());
        return;
    }
    egui::ScrollArea::vertical().id_salt("trellis-palette-list").max_height(LIST_MAX_H).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, shown) in list.iter().enumerate() {
            let selected = sel == Some(i);
            let r = row(ui, &shown.entry.label, shown.entry.detail.as_deref(), &shown.matched, 0, selected);
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

/// A sub-list's items matching the query (at most `MAX_PICK_ROWS`). Returns the footer.
fn pick_list(ui: &mut Ui, p: &mut Palette, msgs: &mut Vec<Msg>, run_selected: bool) -> &'static str {
    let (selected, mut scroll) = (p.selected, p.scroll_to_selected);
    let Mode::Pick(pk) = &mut p.mode else { return "" };
    pk.refresh(&p.query);
    let kind = pk.kind;
    match &pk.items {
        Items::Loading => note(ui, kind.loading(), color::weak()),
        Items::Failed(e) => note(ui, e, color::red()),
        Items::Ready(v) if v.is_empty() => note(ui, kind.empty_text(), color::weak()),
        Items::Ready(_) => match pk.ranked() {
            Some(r) if !r.hits.is_empty() => {
                let sel = selected.min(r.hits.len() - 1);
                if run_selected {
                    msgs.push(Msg::RunPaletteCommand(kind.command(pk.label(r.hits[sel].index))));
                }
                egui::ScrollArea::vertical().id_salt("trellis-palette-items").max_height(LIST_MAX_H).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, hit) in r.hits.iter().enumerate() {
                        let label = pk.label(hit.index);
                        // The folder part (through the last '/') is dimmed.
                        let dim = label.rfind('/').map_or(0, |b| label[..=b].chars().count());
                        let resp = row(ui, label, None, &hit.matched, dim, i == sel);
                        if resp.clicked() {
                            msgs.push(Msg::RunPaletteCommand(kind.command(label)));
                        }
                        if i == sel && scroll {
                            resp.scroll_to_me(None);
                            scroll = false;
                        }
                    }
                });
                if r.total > r.hits.len() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(kind.more_text(r.hits.len(), r.total)).size(12.0).color(color::weak()));
                }
            }
            _ => note(ui, kind.none_text(), color::weak()),
        },
    }
    p.scroll_to_selected = scroll;
    kind.footer()
}

/// One line of text in place of the list.
fn note(ui: &mut Ui, text: &str, col: Color32) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(col));
    ui.add_space(4.0);
}

/// One row: the label with its matched characters in the accent colour (the chars before
/// `dim_before` dimmed otherwise), and the detail in dim text at the right.
fn row(ui: &mut Ui, label: &str, detail: Option<&str>, matched: &[usize], dim_before: usize, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 3.0, color::selected());
    } else if resp.hovered() {
        p.rect_filled(rect, 3.0, color::hover());
    }
    let mut right_w = 0.0;
    if let Some(detail) = detail {
        let g = p.layout_no_wrap(detail.to_owned(), FontId::proportional(12.0), color::weak());
        right_w = g.size().x + 18.0;
        p.galley(pos2(rect.right() - g.size().x - 10.0, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    let g = ui.fonts(|f| f.layout_job(label_job(label, matched, dim_before)));
    let text_rect = Rect::from_min_max(pos2(rect.left() + 10.0, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    p.with_clip_rect(text_rect).galley(pos2(text_rect.left(), rect.center().y - g.size().y / 2.0), g, color::text());
    resp
}

/// The label as runs of one colour each: matched chars in the accent colour, the chars before
/// `dim_before` dimmed, the rest in the text colour.
fn label_job(label: &str, matched: &[usize], dim_before: usize) -> LayoutJob {
    let mut job = LayoutJob::default();
    let format = |col: Color32| egui::TextFormat { font_id: FontId::proportional(LABEL_SIZE), color: col, ..Default::default() };
    let colour = |i: usize| {
        if matched.contains(&i) {
            color::accent()
        } else if i < dim_before {
            color::weak()
        } else {
            color::text()
        }
    };
    let mut run = String::new();
    let mut run_col = color::text();
    for (i, c) in label.chars().enumerate() {
        let col = colour(i);
        if col != run_col && !run.is_empty() {
            job.append(&std::mem::take(&mut run), 0.0, format(run_col));
        }
        run_col = col;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, format(run_col));
    }
    job
}
