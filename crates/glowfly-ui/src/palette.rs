//! The command palette (Ctrl+P): a search field over the commands available now, ranked by a
//! fuzzy match, or over a two-step command's items (tracked files for File history, stashes to
//! apply, pop or delete), or a free-text step (the message for a new stash). The lists come
//! from `state::palette`; this file only draws them (with the empty query's group headings) and
//! turns keys and clicks into messages.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::state::AppState;
use crate::state::palette::{self, InputKind, Items, Mode, Palette};
use crate::state::palette_nav::{Jump, PAGE_ROWS};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Modifiers, Rect, Response, RichText, Sense, Ui, WidgetInfo, WidgetType, pos2, text::LayoutJob,
    vec2,
};

const WIDTH: f32 = 560.0;
const ROW_H: f32 = 28.0;
const LIST_MAX_H: f32 = 360.0;
/// A group heading's height (Recent, Suggested, All commands).
const GROUP_HEADING_H: f32 = 22.0;
const LABEL_SIZE: f32 = type_scale::BODY;
// A page (PageUp/PageDown) is the rows the list shows at its full height.
const _: () = assert!(PAGE_ROWS == (LIST_MAX_H / ROW_H) as usize);

/// The id of the palette's modal area (its rect is in egui's memory under this id).
pub(crate) const AREA_ID: &str = "glowfly-palette";

/// The palette's rect on screen while it is open (tests and the smoke probe find its rows
/// inside it).
pub fn area_rect(ctx: &egui::Context) -> Option<Rect> {
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new(AREA_ID));
    ctx.memory(|m| if m.areas().visible_last_frame(&layer) { m.area_rect(layer.id) } else { None })
}

/// Draws the open palette. A credential prompt above it keeps the keyboard.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(p) = st.palette.as_ref() else { return };
    let stepping = !matches!(p.mode, Mode::Commands);
    let empty = p.query.is_empty();
    let prompting = st.askpass.showing();
    // The free-text step has no list, so the page and end keys stay with the field.
    let listing = !matches!(p.mode, Mode::Input(_));
    let id = Id::new(AREA_ID);
    let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, vec2(0.0, 80.0));
    let resp = super::theme::show_modal(ctx, super::theme::modal(ctx, id).area(area), |ui| {
        ui.set_width(WIDTH);
        let mut run_selected = false;
        if !prompting {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    msgs.push(Msg::Palette(PaletteMsg::PaletteMove(1)));
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    msgs.push(Msg::Palette(PaletteMsg::PaletteMove(-1)));
                }
                if listing {
                    // Plain Home and End move the text cursor once there is a query.
                    let end = |i: &mut egui::InputState, key| {
                        i.consume_key(Modifiers::COMMAND, key) || (empty && i.consume_key(Modifiers::NONE, key))
                    };
                    let jumps = [
                        (i.consume_key(Modifiers::NONE, Key::PageDown), Jump::PageDown),
                        (i.consume_key(Modifiers::NONE, Key::PageUp), Jump::PageUp),
                        (end(i, Key::Home), Jump::First),
                        (end(i, Key::End), Jump::Last),
                    ];
                    for (_, j) in jumps.into_iter().filter(|(pressed, _)| *pressed) {
                        msgs.push(Msg::Palette(PaletteMsg::PaletteJump(j)));
                    }
                }
                run_selected = i.consume_key(Modifiers::NONE, Key::Enter);
                // In a second step, Backspace on an empty field and Esc go back to the commands.
                // Taken before the field draws, and before the modal would close on Esc.
                let back = |i: &mut egui::InputState| {
                    (empty && i.consume_key(Modifiers::NONE, Key::Backspace)) | i.consume_key(Modifiers::NONE, Key::Escape)
                };
                if stepping && back(i) {
                    msgs.push(Msg::Palette(PaletteMsg::PaletteBack));
                }
            });
        }
        let Some(p) = st.palette.as_mut() else { return };
        search_row(ui, p, prompting, msgs);
        ui.add_space(6.0);
        let footer = match &p.mode {
            Mode::Commands => {
                commands_list(ui, st, msgs, run_selected);
                "Up/Down to choose, Enter to run, Esc to close"
            }
            Mode::Pick(_) => pick_list(ui, p, msgs, run_selected),
            &Mode::Input(kind) => input_step(ui, kind, &p.query, msgs, run_selected),
        };
        ui.add_space(6.0);
        ui.label(RichText::new(footer).size(type_scale::SMALL).color(color::weak()));
    });
    if resp.should_close() && !prompting {
        msgs.push(Msg::Palette(PaletteMsg::ClosePalette));
    }
}

/// The search field, after a search icon, or in a second step after a back arrow and its title.
fn search_row(ui: &mut Ui, p: &mut Palette, prompting: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let hint = match &p.mode {
            Mode::Commands => {
                let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Search, color::weak());
                "Type a command, branch or tag name"
            }
            Mode::Pick(pk) => {
                back_and_title(ui, pk.kind.title(), msgs);
                pk.kind.prompt()
            }
            Mode::Input(k) => {
                back_and_title(ui, k.title(), msgs);
                k.prompt()
            }
        };
        let r = controls::field(
            ui,
            egui::TextEdit::singleline(&mut p.query)
                .id(Id::new("glowfly-palette-query"))
                .hint_text(hint)
                .desired_width(f32::INFINITY)
                .font(type_scale::font(type_scale::SUBHEAD)),
        );
        if !prompting {
            r.request_focus();
        }
        if r.changed() {
            p.selected = 0;
        }
    });
}

/// A second step's back arrow (going back when clicked) and its title.
fn back_and_title(ui: &mut Ui, title: &str, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    let col = if r.hovered() { color::text() } else { color::weak() };
    widgets::paint_icon(ui.painter(), rect.center(), Icon::Back, col);
    if r.on_hover_text("Back to commands (Backspace)").clicked() {
        msgs.push(Msg::Palette(PaletteMsg::PaletteBack));
    }
    ui.label(RichText::new(title).size(type_scale::SMALL).color(color::weak()));
}

/// A free-text step: a note under the field; Enter runs the command for the typed text, unless
/// it is blank. Returns the footer.
fn input_step(ui: &mut Ui, kind: InputKind, query: &str, msgs: &mut Vec<Msg>, run_selected: bool) -> &'static str {
    match kind.problem(query) {
        Some(p) => note(ui, p, color::removed()),
        None => note(ui, kind.note(), color::weak()),
    }
    if run_selected && let Some(cmd) = kind.command(query) {
        msgs.push(Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
    }
    kind.footer()
}

/// The commands matching the query (at most `MAX_COMMAND_ROWS`).
fn commands_list(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>, run_selected: bool) {
    if st.palette.is_none() {
        return;
    }
    let listed = palette::listed_open(st);
    let list = &listed.rows;
    let t = st.settings.lang();
    let Some(p) = st.palette.as_mut() else { return };
    let sel = p.selected_in(list.len());
    if run_selected && let Some(s) = sel {
        msgs.push(Msg::Palette(PaletteMsg::RunPaletteCommand(list[s].entry.command.clone())));
    }
    if list.is_empty() {
        note(ui, "No matching commands", color::weak());
        return;
    }
    egui::ScrollArea::vertical().id_salt("glowfly-palette-list").max_height(LIST_MAX_H).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, shown) in list.iter().enumerate() {
            if let Some(&(g, _)) = listed.groups.iter().find(|&&(_, first)| first == i) {
                group_heading(ui, g.title(t));
            }
            let selected = sel == Some(i);
            let e = &shown.entry;
            let key = e.shortcut(&st.keymap).or_else(|| st.custom_commands.key_hint(&e.command));
            let r = row(ui, &e.label, e.detail.as_deref(), key, &shown.matched, 0, selected);
            if r.clicked() {
                msgs.push(Msg::Palette(PaletteMsg::RunPaletteCommand(shown.entry.command.clone())));
            }
            hover_selects(ui, &r, selected, i, msgs);
            if selected && p.scroll_to_selected {
                r.scroll_to_me(None);
                p.scroll_to_selected = false;
            }
        }
    });
    if listed.total > list.len() {
        ui.add_space(4.0);
        let more = format!("Showing {} of {} commands. Type more to narrow the list.", list.len(), listed.total);
        ui.label(RichText::new(more).size(type_scale::SMALL).color(color::weak()));
    }
}

/// A group's small weak heading over its commands. It isn't a row: it can't be selected or run.
fn group_heading(ui: &mut Ui, title: &str) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_HEADING_H), Sense::hover());
    let font = type_scale::font(type_scale::SMALL);
    ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, title, font, color::weak());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, title));
}

/// A sub-list's items matching the query (at most `MAX_PICK_ROWS`). Returns the footer.
fn pick_list(ui: &mut Ui, p: &mut Palette, msgs: &mut Vec<Msg>, run_selected: bool) -> &'static str {
    let (selected, mut scroll) = (p.selected, p.scroll_to_selected);
    let Mode::Pick(pk) = &mut p.mode else { return "" };
    pk.refresh(&p.query);
    let kind = pk.kind;
    match &pk.items {
        Items::Loading => note(ui, kind.loading(), color::weak()),
        Items::Failed(e) => note(ui, e, color::removed()),
        Items::Ready(v) if v.is_empty() => note(ui, kind.empty_text(), color::weak()),
        Items::Ready(_) => match pk.ranked() {
            Some(r) if !r.hits.is_empty() => {
                let sel = selected.min(r.hits.len() - 1);
                if run_selected && let Some(c) = pk.command(r.hits[sel].index) {
                    msgs.push(Msg::Palette(PaletteMsg::RunPaletteCommand(c)));
                }
                egui::ScrollArea::vertical().id_salt("glowfly-palette-items").max_height(LIST_MAX_H).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, hit) in r.hits.iter().enumerate() {
                        let label = pk.label(hit.index);
                        // A file's folder part (through the last '/') is dimmed.
                        let dim = if kind.dims_folders() { label.rfind('/').map_or(0, |b| label[..=b].chars().count()) } else { 0 };
                        let detail = pk.detail(hit.index);
                        let resp = row(ui, label, detail.as_deref(), None, &hit.matched, dim, i == sel);
                        if resp.clicked()
                            && let Some(c) = pk.command(hit.index)
                        {
                            msgs.push(Msg::Palette(PaletteMsg::RunPaletteCommand(c)));
                        }
                        hover_selects(ui, &resp, i == sel, i, msgs);
                        if i == sel && scroll {
                            resp.scroll_to_me(None);
                            scroll = false;
                        }
                    }
                });
                if r.total > r.hits.len() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(kind.more_text(r.hits.len(), r.total)).size(type_scale::SMALL).color(color::weak()));
                }
            }
            _ => note(ui, kind.none_text(), color::weak()),
        },
    }
    p.scroll_to_selected = scroll;
    kind.footer()
}

/// Selects row `i` when the pointer moves onto it. Only real pointer movement counts: a list
/// scrolling under a still pointer (by the wheel or to a keyboard selection) changes nothing.
fn hover_selects(ui: &Ui, r: &Response, selected: bool, i: usize, msgs: &mut Vec<Msg>) {
    if r.hovered() && !selected && ui.input(|inp| inp.pointer.delta() != egui::Vec2::ZERO) {
        msgs.push(Msg::Palette(PaletteMsg::PaletteHover(i)));
    }
}

/// One line of text in place of the list.
fn note(ui: &mut Ui, text: &str, col: Color32) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(col));
    ui.add_space(4.0);
}

/// One row: the label with its matched characters in the accent colour (the chars before
/// `dim_before` dimmed otherwise), then at the right in dim text the detail and, at the far
/// right, the shortcut. The label is clipped before both.
fn row(
    ui: &mut Ui,
    label: &str,
    detail: Option<&str>,
    shortcut: Option<&str>,
    matched: &[usize],
    dim_before: usize,
    selected: bool,
) -> Response {
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
    if let Some(keys) = shortcut {
        let g = p.layout_no_wrap(keys.to_owned(), type_scale::font(type_scale::SMALL), color::weak());
        right_w = g.size().x + 18.0;
        p.galley(pos2(rect.right() - g.size().x - 10.0, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    if let Some(detail) = detail {
        let g = p.layout_no_wrap(detail.to_owned(), type_scale::font(type_scale::SMALL), color::weak());
        // 10px in from the row's edge, or 14px left of the shortcut (`right_w` already holds
        // 8px of space to its left).
        let gap = if shortcut.is_some() { 6.0 } else { 10.0 };
        let x = rect.right() - right_w - g.size().x - gap;
        right_w += g.size().x + gap + 8.0;
        p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color::weak());
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
