//! The welcome's own dialog header (`Dialog::header`): a band tinted toward the accent across the
//! dialog's whole width, painted into the frame's padding, with the title, a subtitle or
//! greeting, the lattice drawing (`welcome_art.rs`) and the shared close button; under it a
//! four-segment progress bar with each step's caption, then the step's title.

use super::dialog_frame;
use super::theme::{self, color};
use super::welcome_art;
use super::widgets;
use crate::state::onboarding::WelcomeStep;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{Align, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;

/// The band's height, from the dialog's top edge.
const BAND_H: f32 = 104.0;
/// Between the band, the progress bar and the step title.
const GAP: f32 = 20.0;
/// The title's inset from the band's left edge and top.
const TEXT_X: f32 = 24.0;
const TEXT_Y: f32 = 28.0;
/// The drawing: at most this wide, ending this far before the right edge (clear of the close
/// box, whose left edge is 48 px in),
/// left out with less room than `ART_MIN_W`, and kept this far from the text.
const ART_MAX_W: f32 = 320.0;
const ART_END: f32 = 56.0;
const ART_MIN_W: f32 = 120.0;
const ART_GAP: f32 = 16.0;
/// The progress segments: gap, height, corners, and the caption's distance below.
const SEG_GAP: f32 = 8.0;
const SEG_H: f32 = 4.0;
const SEG_RADIUS: u8 = radius::XS;
const CAPTION_GAP: f32 = 6.0;

/// The progress row's height: a segment, the gap and a caption line.
fn progress_height(ui: &Ui) -> f32 {
    SEG_H + CAPTION_GAP + ui.fonts(|f| f.row_height(&type_scale::font(type_scale::SMALL)))
}

/// The dialog's header: the band, the progress bar and the step's title. `greeting` is the name
/// git holds, once the identity step is behind.
pub(super) fn draw(ui: &mut Ui, t: Lang, step: WelcomeStep, greeting: Option<&str>) {
    band(ui, t, greeting);
    ui.add_space(GAP);
    progress(ui, t, step);
    ui.add_space(GAP);
    ui.label(RichText::new(step.title(t)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::strong()));
}

/// The band across the dialog's whole width, painted into the frame's padding on the left, top
/// and right, with its top corners rounded to the frame's.
fn band(ui: &mut Ui, t: Lang, greeting: Option<&str>) {
    let pad = f32::from(theme::DIALOG_PAD);
    let (left, top) = (ui.cursor().left(), ui.cursor().top());
    let width = ui.available_width();
    // A dialog's content isn't clipped to its padding, so the band reaches the frame's edges.
    let band = Rect::from_min_max(pos2(left - pad, top - pad), pos2(left + width + pad, top - pad + BAND_H));
    ui.allocate_exact_size(vec2(width, BAND_H - pad), Sense::hover());
    let style = welcome_art::art_style(&color::current());
    let painter = ui.painter().clone();
    painter.rect_filled(band, radius::top(radius::inside(radius::XL, 1)), style.band);
    if let Some(rule) = style.rule {
        let y = band.bottom() - 0.5;
        painter.line_segment([pos2(band.left(), y), pos2(band.right(), y)], Stroke::new(1.0_f32, rule));
    }
    let title_font = type_scale::strong(ui, type_scale::TITLE);
    let subtitle = match greeting {
        Some(name) => t.trf("welcome.greeting", &[("name", &name)]),
        None => t.tr("welcome.subtitle").to_owned(),
    };
    // The text column's natural width decides how much room the drawing gets.
    let width_of = |text: &str, font: FontId| painter.layout_no_wrap(text.to_owned(), font, color::text()).size().x;
    let text_w = width_of(t.tr("welcome.title"), title_font.clone()).max(width_of(&subtitle, type_scale::font(type_scale::BODY)));
    let art_right = band.right() - ART_END;
    let room = art_right - (band.left() + TEXT_X + text_w + ART_GAP);
    let art = (room >= ART_MIN_W).then(|| {
        let left = art_right - room.min(ART_MAX_W);
        Rect::from_min_max(pos2(left, band.top()), pos2(art_right, band.bottom()))
    });
    if let Some(r) = art {
        welcome_art::paint(&painter.with_clip_rect(r), r, &style);
    }
    let text_max = match art {
        Some(r) => r.left() - band.left() - TEXT_X - ART_GAP,
        None => band.width() - TEXT_X - ART_END,
    };
    let text_rect = Rect::from_min_size(band.min + vec2(TEXT_X, TEXT_Y), vec2(text_max.max(0.0), BAND_H - TEXT_Y));
    let mut col = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)));
    col.spacing_mut().item_spacing.y = 4.0;
    col.label(RichText::new(t.tr("welcome.title")).font(title_font).color(color::text()));
    widgets::truncate_label(&mut col, RichText::new(subtitle).color(color::weak()), text_max);
    // Where every dialog has it; welcome.rs reads the click through `take_closed`.
    dialog_frame::close_button(ui, t.tr("welcome.close.tip"), left + width, top);
}

/// Four equal segments filled with the accent up to the current step, each captioned under its
/// left end, read out as one "Step N of 4" progress indicator.
pub(super) fn progress(ui: &mut Ui, t: Lang, step: WelcomeStep) {
    let height = progress_height(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let current_font = type_scale::strong(ui, type_scale::SMALL);
    let p = ui.painter();
    let n = WelcomeStep::ALL.len() as f32;
    let seg_w = ((rect.width() - (n - 1.0) * SEG_GAP) / n).max(0.0);
    let caption_y = rect.top() + SEG_H + CAPTION_GAP + (height - SEG_H - CAPTION_GAP) / 2.0;
    for s in WelcomeStep::ALL {
        let left = rect.left() + s.index() as f32 * (seg_w + SEG_GAP);
        let seg = Rect::from_min_size(pos2(left, rect.top()), vec2(seg_w, SEG_H));
        let fill = if s.index() <= step.index() { color::accent() } else { color::border() };
        p.rect_filled(seg, CornerRadius::same(SEG_RADIUS), fill);
        let (font, text) = match s.index().cmp(&step.index()) {
            std::cmp::Ordering::Less => (type_scale::font(type_scale::SMALL), color::weak()),
            std::cmp::Ordering::Equal => (current_font.clone(), color::text()),
            std::cmp::Ordering::Greater => (type_scale::font(type_scale::SMALL), color::faint()),
        };
        widgets::paint_cut_text(p, pos2(left, caption_y), s.caption(t), font, text, seg_w);
    }
    let total = WelcomeStep::ALL.len();
    let label = t.trf("welcome.progress.label", &[("number", &(step.index() + 1)), ("total", &total), ("step", &step.caption(t))]);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, &label));
}
