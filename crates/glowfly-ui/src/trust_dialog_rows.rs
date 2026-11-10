//! The trust dialog's finding rows and its path line. Each setting or hook row is a mark
//! column (an amber warning triangle for what Glowfly doesn't recognise, a green tick for Git
//! LFS's standard setup), then the row's text, wrapping under itself: a setting as its key and
//! value with the program word emphasised (`trust::rows::value_parts`), a hook as its name, and
//! on a recognised row the "Git LFS" tag after the text. How a row looks is all recognition
//! changes; what is trusted stays the same.

use super::theme::color;
use super::theme::type_scale;
use super::widgets::severity::{self, Severity};
use super::widgets::truncate::cut_path_in;
use super::widgets::{self, Icon, icon};
use crate::theme::sizes;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, RichText, Sense, TextStyle, TextWrapMode, Ui, WidgetInfo, WidgetType};
use glowfly_core::i18n::Lang;
use glowfly_model::state::trust::rows::value_parts;
use std::sync::Arc;

/// The mark column: as wide as the Hooks heading's icon, so the rows' text lines up with its
/// text.
const MARK_W: f32 = icon::SIZE_MD;
/// Between the mark column and the row's text, as between the heading's icon and its text.
const MARK_GAP: f32 = 6.0;
/// Between a recognised row's text and its tag.
const TAG_GAP: f32 = 8.0;

/// What a row's mark says.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    /// Not recognised, with `tip` (translated) as its tooltip.
    Unknown { tip: &'static str },
    /// Git LFS's own standard setting or hook.
    GitLfs,
}

/// The repository's path on one line, `weak`: cut at whole folders to the body's width (the
/// whole path as its tooltip then). Its accessible name is always the whole path.
pub(super) fn path_line(ui: &mut Ui, whole: &str) {
    let font = TextStyle::Body.resolve(ui.style());
    let shown = cut_path_in(ui.painter(), whole, &font, ui.available_width(), std::path::MAIN_SEPARATOR);
    let cut = shown != whole;
    let resp = ui.add(egui::Label::new(RichText::new(shown).color(color::weak())).wrap_mode(TextWrapMode::Extend));
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, whole));
    if cut {
        resp.on_hover_text(whole);
    }
}

/// A setting's row: `key = value`, the key and ` = ` in `weak`, the value's program word in
/// `strong` and the rest of it in `text` (all of it in `text` when it has no program word).
pub(super) fn setting(ui: &mut Ui, t: Lang, key: &str, value: &str, mark: Mark) {
    let font = TextStyle::Monospace.resolve(ui.style());
    let mut job = LayoutJob::default();
    let mut add = |s: &str, c: Color32| {
        if !s.is_empty() {
            job.append(s, 0.0, TextFormat::simple(font.clone(), c));
        }
    };
    add(key, color::weak());
    add(" = ", color::weak());
    match value_parts(value) {
        Some(r) => {
            add(&value[..r.start], color::text());
            add(&value[r.clone()], color::strong());
            add(&value[r.end..], color::text());
        }
        None => add(value, color::text()),
    }
    row(ui, t, job, mark);
}

/// A hook's row, or a step's: its name (the step as written) in mono `text`.
pub(super) fn hook(ui: &mut Ui, t: Lang, name: &str, mark: Mark) {
    let job = LayoutJob::simple_singleline(name.to_owned(), TextStyle::Monospace.resolve(ui.style()), color::text());
    row(ui, t, job, mark);
}

/// The mark column, then `job` wrapping at the text column, then a recognised row's tag on the
/// text's last line (or under it, where it doesn't fit there).
fn row(ui: &mut Ui, t: Lang, mut job: LayoutJob, mark: Mark) {
    let text_w = (ui.available_width() - MARK_W - MARK_GAP).max(MARK_W);
    job.wrap.max_width = text_w;
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = MARK_GAP;
        let first_h = galley.rows.first().map_or(galley.size().y, |r| r.rect.height());
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(MARK_W, first_h), Sense::hover());
        let c = rect.center();
        match mark {
            Mark::Unknown { tip } => {
                severity::paint(ui.painter(), c, icon::SIZE_SM, Severity::Warning, color::warn());
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, t.tr("trust.unknown.name")));
                resp.on_hover_text(tip);
            }
            Mark::GitLfs => {
                widgets::paint_icon_sized(ui.painter(), c, Icon::Check, color::added(), icon::SIZE_SM);
                resp.on_hover_text(t.tr("trust.known.lfs.tip"));
            }
        }
        let text = ui.add(egui::Label::new(Arc::clone(&galley)));
        if mark == Mark::GitLfs {
            tag(ui, t, &galley, text.rect, text_w);
        }
    });
}

/// "Git LFS" in SMALL `weak` text, `TAG_GAP` after the end of the text at `at` when it fits in
/// the text column (`text_w` wide), else at the column's start under it.
fn tag(ui: &mut Ui, t: Lang, galley: &egui::Galley, at: egui::Rect, text_w: f32) {
    let words = t.tr("trust.known.lfs");
    let size = ui.fonts(|f| f.layout_no_wrap(words.to_owned(), type_scale::font(type_scale::SMALL), color::weak())).size();
    let last = galley.rows.last().map_or(egui::Rect::ZERO, |r| r.rect);
    let fits = last.max.x + TAG_GAP + size.x <= text_w;
    let text = RichText::new(words).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    let pos = if fits {
        egui::pos2(at.min.x + last.max.x + TAG_GAP, sizes::centred_on(at.min.y + last.center().y, size.y))
    } else {
        egui::pos2(at.min.x, at.max.y)
    };
    let resp = ui.put(egui::Rect::from_min_size(pos, size), egui::Label::new(text).wrap_mode(TextWrapMode::Extend));
    resp.on_hover_text(t.tr("trust.known.lfs.tip"));
}
