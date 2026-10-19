//! The header's left side: the file-type icon, the folder (cut from the left when short of
//! room) and the name, the status chip (the letter, then the added and removed lines or the
//! sizes) and the context chip (the commit, Unstaged, Staged or the compared range).

use super::controls::ICON;
use super::fit::{Plan, Widths};
use crate::state::diff_header::{ChangeText, Context, FileKind, HeaderModel};
use crate::ui::common::status_color;
use crate::ui::theme::{color, fonts};
use crate::ui::widgets::truncate::cut_left;
use crate::ui::widgets::{Icon, paint_icon_sized};
use eframe::egui::{self, Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::lfs::pointer::size_text;

/// The gap between the left-hand parts.
pub(super) const GAP: f32 = 8.0;
const PATH_PX: f32 = 14.0;
/// A chip's height, corners, side padding and the gap between its parts.
const CHIP_H: f32 = 22.0;
const CHIP_RADIUS: u8 = 6;
const CHIP_PAD: f32 = 8.0;
const CHIP_GAP: f32 = 6.0;
const CHIP_PX: f32 = 12.0;
/// The painted arrow between two sizes.
const ARROW_W: f32 = 8.0;

/// One piece of a chip.
enum Part {
    Text(String, FontId, Color32),
    /// The arrow from the old size to the new.
    Arrow,
}

fn folder_font() -> FontId {
    FontId::proportional(PATH_PX)
}

fn chip_font() -> FontId {
    FontId::proportional(CHIP_PX)
}

fn width_of(ui: &Ui, text: &str, font: FontId) -> f32 {
    super::controls::text_w(ui, text, font)
}

fn status_parts(m: &HeaderModel) -> Vec<Part> {
    let mut v = Vec::new();
    if let Some(s) = m.status {
        v.push(Part::Text(s.to_string(), chip_font(), status_color(s)));
    }
    match m.change {
        ChangeText::Lines { added, removed } => {
            v.push(Part::Text(format!("+{added}"), chip_font(), color::green()));
            v.push(Part::Text(format!("\u{2212}{removed}"), chip_font(), color::red()));
        }
        ChangeText::Sizes { old, new } => match (old, new) {
            (Some(a), Some(b)) => {
                v.push(Part::Text(size_text(a), chip_font(), color::text()));
                v.push(Part::Arrow);
                v.push(Part::Text(size_text(b), chip_font(), color::text()));
            }
            (Some(s), None) | (None, Some(s)) => v.push(Part::Text(size_text(s), chip_font(), color::text())),
            (None, None) => {}
        },
        ChangeText::None => {}
    }
    v
}

fn context_parts(m: &HeaderModel) -> Vec<Part> {
    let (text, font) = match &m.context {
        Context::Commit { short, .. } => (short.clone(), FontId::monospace(CHIP_PX)),
        Context::Unstaged => ("Unstaged".into(), chip_font()),
        Context::Staged => ("Staged".into(), chip_font()),
        Context::Between(r) | Context::ToWorkdir(r) => (r.clone(), chip_font()),
    };
    vec![Part::Text(text, font, color::text())]
}

fn parts_text(parts: &[Part]) -> String {
    let words: Vec<&str> = parts
        .iter()
        .map(|p| match p {
            Part::Text(t, ..) => t.as_str(),
            Part::Arrow => "to",
        })
        .collect();
    words.join(" ")
}

fn chip_width(ui: &Ui, parts: &[Part]) -> f32 {
    if parts.is_empty() {
        return 0.0;
    }
    let inner: f32 = parts
        .iter()
        .map(|p| match p {
            Part::Text(t, f, _) => width_of(ui, t, f.clone()),
            Part::Arrow => ARROW_W,
        })
        .sum();
    2.0 * CHIP_PAD + inner + (parts.len() - 1) as f32 * CHIP_GAP
}

/// Fills in the left-hand widths of `w`.
pub(super) fn widths(ui: &Ui, m: &HeaderModel, w: &mut Widths) {
    w.icon = ICON;
    w.gap = GAP;
    w.folder = if m.folder.is_empty() { 0.0 } else { width_of(ui, &m.folder, folder_font()) };
    w.name = width_of(ui, &m.name, fonts::semibold(ui, PATH_PX));
    w.status = chip_width(ui, &status_parts(m));
    w.context = chip_width(ui, &context_parts(m));
}

/// Draws the left-hand parts as `plan` has them.
pub(super) fn show(ui: &mut Ui, m: &HeaderModel, w: &Widths, plan: &Plan) {
    ui.spacing_mut().item_spacing.x = 0.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
    let icon = match m.kind {
        FileKind::Image => Icon::FileImage,
        FileKind::Binary => Icon::FileBinary,
        FileKind::Code => Icon::FileCode,
        FileKind::Text => Icon::File,
    };
    paint_icon_sized(ui.painter(), rect.center(), icon, color::weak(), ICON);
    ui.add_space(GAP);
    if plan.folder_w > 0.0 {
        let shown = cut_left(ui, &m.folder, &folder_font(), plan.folder_w);
        let cut = shown != m.folder;
        let r = ui.add(egui::Label::new(egui::RichText::new(shown).font(folder_font()).color(color::weak())).selectable(false).extend());
        if cut {
            r.on_hover_text(&m.path);
        }
    }
    let name = egui::RichText::new(&m.name).font(fonts::semibold(ui, PATH_PX)).color(color::text());
    if plan.name_w + 0.5 < w.name {
        let (r, cut) = crate::ui::widgets::cut_label(ui, name, plan.name_w);
        if cut {
            r.on_hover_text(&m.path);
        }
    } else {
        ui.add(egui::Label::new(name).selectable(false).extend());
    }
    if plan.status && w.status > 0.0 {
        ui.add_space(GAP);
        chip(ui, &status_parts(m));
    }
    if plan.context && w.context > 0.0 {
        ui.add_space(GAP);
        let r = chip(ui, &context_parts(m));
        if let Context::Commit { summary: Some(s), .. } = &m.context {
            r.on_hover_text(s);
        }
    }
}

/// A pill of `parts` on the toolbar fill, named by its text.
fn chip(ui: &mut Ui, parts: &[Part]) -> egui::Response {
    let width = chip_width(ui, parts);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, CHIP_H), Sense::hover());
    let text = parts_text(parts);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(CHIP_RADIUS), color::toolbar());
    let mut x = rect.left() + CHIP_PAD;
    let y = rect.center().y;
    for part in parts {
        match part {
            Part::Text(t, f, c) => {
                let g = p.layout_no_wrap(t.clone(), f.clone(), *c);
                let size = g.size();
                p.galley(pos2(x, y - size.y / 2.0), g, *c);
                x += size.x;
            }
            Part::Arrow => {
                arrow(p, Rect::from_min_size(pos2(x, y - 4.0), vec2(ARROW_W, 8.0)), color::weak());
                x += ARROW_W;
            }
        }
        x += CHIP_GAP;
    }
    resp
}

/// A small right-pointing arrow filling `r`.
fn arrow(p: &egui::Painter, r: Rect, c: Color32) {
    let s = Stroke::new(1.3_f32, c);
    let y = r.center().y;
    p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], s);
    p.line_segment([pos2(r.right() - 3.5, y - 3.5), pos2(r.right(), y)], s);
    p.line_segment([pos2(r.right() - 3.5, y + 3.5), pos2(r.right(), y)], s);
}
