//! Draws Markdown bodies (pull request and issue descriptions and comments) parsed by
//! `glowfly_core::markdown` and kept in `MarkdownCache`: headings, paragraphs, code blocks
//! (`markdown_code.rs`), lists with painted bullets, numbers and task boxes (the fonts lack
//! reliable bullet glyphs), quotes, tables and rules. Text runs are drawn by
//! `markdown_inline.rs`.

use super::markdown_cache::{CodeBlock, MarkdownCache, Rendered};
use super::markdown_inline::{self, Look};
use super::theme::{color, type_scale};
use super::widgets::{self, Icon};
use crate::msg::Msg;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::markdown::{Block, Inline, Item};
use std::cell::Cell;

/// The width of a list item's bullet, number or task box column.
const MARKER_W: f32 = 24.0;
/// How far a quote's text sits right of its bar.
const QUOTE_INDENT: f32 = 14.0;

/// Where a block is drawn: its text color, how deep in lists it is, and the body's code blocks
/// with the next one to draw.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    color: Color32,
    lists: usize,
    code: &'a [CodeBlock],
    next_code: &'a Cell<usize>,
}

/// Draws `text` as Markdown at the width of `ui`, under `id` (unique among the bodies shown
/// together, so their code blocks and tables keep their own scroll). Text too long to parse is
/// drawn plain.
pub(in crate::ui) fn body(ui: &mut Ui, id: impl std::hash::Hash, text: &str, cache: &mut MarkdownCache, msgs: &mut Vec<Msg>) {
    let r = cache.get(text);
    ui.push_id(id, |ui| show(ui, &r, text, msgs));
}

fn show(ui: &mut Ui, r: &Rendered, text: &str, msgs: &mut Vec<Msg>) {
    let Some(doc) = &r.doc else {
        ui.add(egui::Label::new(RichText::new(text).color(color::text())).wrap());
        return;
    };
    let next = Cell::new(0);
    let cx = Ctx { color: color::text(), lists: 0, code: &r.code, next_code: &next };
    ui.vertical(|ui| blocks(ui, &doc.blocks, cx, msgs));
}

fn blocks(ui: &mut Ui, list: &[Block], cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for (i, b) in list.iter().enumerate() {
        if i > 0 {
            ui.add_space(if matches!(b, Block::Heading { .. }) { 12.0 } else { 6.0 });
        }
        // Each block under its own id, so two code blocks or tables keep their own scroll.
        ui.push_id(i, |ui| block(ui, b, cx, msgs));
    }
}

fn block(ui: &mut Ui, b: &Block, cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    match b {
        Block::Heading { level, text } => heading(ui, *level, text, cx, msgs),
        Block::Paragraph(text) => markdown_inline::runs(ui, text, look(cx), true, msgs),
        Block::Code { text, .. } => {
            let i = cx.next_code.get();
            cx.next_code.set(i + 1);
            match cx.code.get(i) {
                Some(code) => super::markdown_code::block(ui, &code.text, code.spans.as_deref()),
                None => super::markdown_code::block(ui, text, None),
            }
        }
        Block::List { start, items } => list(ui, *start, items, cx, msgs),
        Block::Quote(inner) => quote(ui, inner, cx, msgs),
        Block::Table { head, rows } => table(ui, head, rows, cx, msgs),
        Block::Rule => {
            ui.add_space(4.0);
            rule(ui);
            ui.add_space(4.0);
        }
    }
}

fn look(cx: Ctx<'_>) -> Look {
    Look { size: type_scale::BODY, strong: false, color: cx.color }
}

/// Level 1 at the title step, 2 at the subhead step, the rest at body size; all SemiBold, and
/// a rule under levels 1 and 2.
fn heading(ui: &mut Ui, level: u8, text: &[Inline], cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    let size = match level {
        1 => type_scale::TITLE,
        2 => type_scale::SUBHEAD,
        _ => type_scale::BODY,
    };
    markdown_inline::runs(ui, text, Look { size, strong: true, ..look(cx) }, true, msgs);
    if level <= 2 {
        ui.add_space(2.0);
        rule(ui);
    }
}

/// A thin line across the width.
fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, color::border()));
}

fn list(ui: &mut Ui, start: Option<u64>, items: &[Item], cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    let inner = Ctx { lists: cx.lists + 1, ..cx };
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            ui.add_space(2.0);
        }
        ui.push_id(i, |ui| {
            ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                let row_h = ui.text_style_height(&egui::TextStyle::Body);
                let (rect, _) = ui.allocate_exact_size(vec2(MARKER_W, row_h), Sense::hover());
                let number = start.map(|s| s.saturating_add(i as u64));
                marker(ui, rect, item.task, number, cx);
                ui.vertical(|ui| blocks(ui, &item.blocks, inner, msgs));
            });
        });
    }
}

/// A task box, a number ("3."), or a bullet that changes with the depth: a dot, a ring, then
/// a small square.
fn marker(ui: &Ui, rect: Rect, task: Option<bool>, number: Option<u64>, cx: Ctx<'_>) {
    let p = ui.painter();
    let c = pos2(rect.left() + 8.0, rect.center().y);
    if let Some(done) = task {
        let r = Rect::from_center_size(pos2(rect.left() + 9.0, c.y), vec2(14.0, 14.0));
        if done {
            p.rect_filled(r, 3.0, color::accent());
            widgets::paint_icon(p, r.center(), Icon::Check, Color32::WHITE);
        } else {
            p.rect_stroke(r, 3.0, Stroke::new(1.0_f32, color::weak()), egui::StrokeKind::Inside);
        }
        return;
    }
    if let Some(n) = number {
        let font = FontId::proportional(type_scale::BODY);
        p.text(pos2(rect.right() - 6.0, c.y), Align2::RIGHT_CENTER, format!("{n}."), font, cx.color);
        return;
    }
    let col = color::weak();
    match cx.lists {
        0 => {
            p.circle_filled(c, 2.5, col);
        }
        1 => {
            p.circle_stroke(c, 2.5, Stroke::new(1.0_f32, col));
        }
        _ => {
            p.rect_filled(Rect::from_center_size(c, vec2(4.0, 4.0)), 0.0, col);
        }
    }
}

/// The quoted blocks in the weak color, indented, with a bar down their left.
fn quote(ui: &mut Ui, inner: &[Block], cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    let quoted = Ctx { color: color::weak(), ..cx };
    let r = ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
        ui.add_space(QUOTE_INDENT);
        ui.vertical(|ui| blocks(ui, inner, quoted, msgs));
    });
    let rect = r.response.rect;
    ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 1.0, color::border());
}

/// A striped grid with the header row strong; scrolls sideways when wider than the column.
fn table(ui: &mut Ui, head: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>], cx: Ctx<'_>, msgs: &mut Vec<Msg>) {
    egui::ScrollArea::horizontal().id_salt("md-table-scroll").auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("md-table").striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
            let strong = Look { strong: true, ..look(cx) };
            for cell in head {
                markdown_inline::runs(ui, cell, strong, false, msgs);
            }
            ui.end_row();
            for row in rows {
                for cell in row {
                    markdown_inline::runs(ui, cell, look(cx), false, msgs);
                }
                ui.end_row();
            }
        });
    });
}
