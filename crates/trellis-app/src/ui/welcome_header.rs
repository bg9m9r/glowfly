//! The welcome's top: a full-width band tinted toward the accent with the title, a subtitle or
//! greeting, the lattice drawing (`welcome_art.rs`) and the close ×, and under it a four-segment
//! progress bar with each step's caption.

use super::theme::color;
use super::welcome_art;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::onboarding::OnboardingMsg;
use crate::state::onboarding::{Welcome, WelcomeStep};
use crate::ui::theme::type_scale;
use eframe::egui::{Align, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;

/// The band's height.
pub(super) const BAND_H: f32 = 104.0;
/// The title's inset from the band's left edge and top.
const TEXT_X: f32 = 24.0;
const TEXT_Y: f32 = 28.0;
/// The close button's inset from the top right, and its size.
const CLOSE_INSET: f32 = 12.0;
const CLOSE_BOX: f32 = 18.0;
/// The drawing: at most this wide, ending this far before the right edge (clear of the ×),
/// left out with less room than `ART_MIN_W`, and kept this far from the text.
const ART_MAX_W: f32 = 320.0;
const ART_END: f32 = 48.0;
const ART_MIN_W: f32 = 120.0;
const ART_GAP: f32 = 16.0;
/// The progress segments: gap, height, corners, and the caption's distance below.
const SEG_GAP: f32 = 8.0;
const SEG_H: f32 = 4.0;
const SEG_RADIUS: u8 = 2;
const CAPTION_GAP: f32 = 6.0;

/// The progress row's height: a segment, the gap and a caption line.
pub(super) fn progress_height(ui: &Ui) -> f32 {
    SEG_H + CAPTION_GAP + ui.fonts(|f| f.row_height(&type_scale::font(type_scale::SMALL)))
}

/// The band across the dialog's whole width, with its corners rounded to the frame's.
pub(super) fn band(ui: &mut Ui, t: Lang, w: &Welcome, msgs: &mut Vec<Msg>) {
    let (band, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAND_H), Sense::hover());
    let style = welcome_art::art_style(&color::current());
    let painter = ui.painter().clone();
    painter.rect_filled(band, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, style.band);
    if let Some(rule) = style.rule {
        let y = band.bottom() - 0.5;
        painter.line_segment([pos2(band.left(), y), pos2(band.right(), y)], Stroke::new(1.0_f32, rule));
    }
    let title_font = type_scale::strong(ui, type_scale::TITLE);
    let subtitle = match w.greeting_name() {
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
    let close = Rect::from_min_size(pos2(band.right() - CLOSE_INSET - CLOSE_BOX, band.top() + CLOSE_INSET), vec2(CLOSE_BOX, CLOSE_BOX));
    let mut corner = ui.new_child(UiBuilder::new().max_rect(close));
    if widgets::small_icon(&mut corner, Icon::Close, t.tr("welcome.close.tip"), color::weak()).clicked() {
        msgs.push(Msg::Onboarding(OnboardingMsg::CloseWelcome));
    }
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
