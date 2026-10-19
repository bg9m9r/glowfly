//! The one banner style: a full-width strip on the `banner` fill with a 3 px stripe in its
//! kind's colour at the left edge, a 16 px icon (or a spinner while something runs), roomy
//! padding, wrapped text and its buttons at the right in the normal styles. The strips under
//! the toolbar use `top`; banners inside a panel use `frame` around their own content, or
//! `show` for a title, text and buttons.

use super::super::theme::color;
use super::severity::{self, Severity};
use super::{Icon, paint_icon};
use eframe::egui::{self, Align, Color32, CursorIcon, InnerResponse, Layout, Rect, Response, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use std::borrow::Cow;

/// The stripe's width at the left edge.
const STRIPE: f32 = 3.0;
/// The icon's square, and the spinner's.
const ICON: f32 = 16.0;
const SPINNER: f32 = 14.0;
/// Content up to this tall is one row, and its icon is centred on it.
const ONE_ROW: f32 = 34.0;

/// What a banner says, which sets its colour and icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    Info,
    Warning,
    Error,
    /// Something is running.
    Progress,
}

impl BannerKind {
    fn color(self) -> Color32 {
        match self {
            BannerKind::Info => color::blue(),
            BannerKind::Warning => color::amber(),
            BannerKind::Error => color::red(),
            BannerKind::Progress => color::accent(),
        }
    }

    fn severity(self) -> Option<Severity> {
        match self {
            BannerKind::Info => Some(Severity::Info),
            BannerKind::Warning => Some(Severity::Warning),
            BannerKind::Error => Some(Severity::Error),
            BannerKind::Progress => None,
        }
    }
}

/// A banner's content for `show`: a strong title, wrapped text and a coloured detail cut to
/// one line, any of them optional.
pub struct Banner<'a> {
    kind: BannerKind,
    title: Option<Cow<'a, str>>,
    text: Option<Cow<'a, str>>,
    detail: Option<(Cow<'a, str>, Color32)>,
    icon: Option<(Icon, Color32)>,
    clickable: bool,
}

impl<'a> Banner<'a> {
    pub fn new(kind: BannerKind) -> Self {
        Self { kind, title: None, text: None, detail: None, icon: None, clickable: false }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn text(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<Cow<'a, str>>, col: Color32) -> Self {
        self.detail = Some((detail.into(), col));
        self
    }

    /// Another icon than the kind's mark, in `col` (a key for signing in, a check when done).
    pub fn icon(mut self, icon: Icon, col: Color32) -> Self {
        self.icon = Some((icon, col));
        self
    }

    /// The whole strip clicks (its buttons stay their own); `show` returns that click.
    pub fn clickable(mut self, on: bool) -> Self {
        self.clickable = on;
        self
    }
}

/// The banner around `content`: fill, stripe, padding and the icon slot (the kind's mark, or
/// `icon` in its colour), as wide as `ui` allows.
pub fn frame<R>(ui: &mut Ui, kind: BannerKind, icon: Option<(Icon, Color32)>, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let r = egui::Frame::new().fill(color::banner()).inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let r = ui.horizontal_top(|ui| {
            let (slot, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
            ui.add_space(4.0);
            let inner = ui.vertical(content);
            (slot, inner)
        });
        let (slot, inner) = r.inner;
        let h = inner.response.rect.height();
        let cy = if h <= ONE_ROW { inner.response.rect.center().y } else { slot.top() + 10.0 };
        let c = pos2(slot.center().x, cy);
        match (icon, kind.severity()) {
            (Some((icon, col)), _) => paint_icon(ui.painter(), c, icon, col),
            (None, Some(s)) => severity::paint(ui.painter(), c, ICON, s, s.color()),
            (None, None) => {
                let spin = Rect::from_center_size(c, vec2(SPINNER, SPINNER));
                egui::Spinner::new().size(SPINNER).color(color::accent()).paint_at(ui, spin);
            }
        }
        inner.inner
    });
    let rect = r.response.rect;
    ui.painter().rect_filled(Rect::from_min_max(rect.left_top(), pos2(rect.left() + STRIPE, rect.bottom())), 0.0, kind.color());
    r
}

/// The banner `b` with `buttons` at the right (added right to left: the first is rightmost).
/// With `b.clickable`, the response is the strip's click.
pub fn show(ui: &mut Ui, b: Banner, buttons: impl FnOnce(&mut Ui)) -> Response {
    let Banner { kind, title, text, detail, icon, clickable } = b;
    let draw = |ui: &mut Ui| {
        frame(ui, kind, icon, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    buttons(ui);
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        if let Some(title) = &title {
                            ui.label(RichText::new(title.as_ref()).strong().color(color::text()));
                        }
                        if let Some(text) = &text {
                            ui.add(egui::Label::new(RichText::new(text.as_ref()).color(color::text())).wrap());
                        }
                        if let Some((detail, col)) = &detail {
                            ui.add(egui::Label::new(RichText::new(detail.as_ref()).color(*col)).truncate());
                        }
                    });
                });
            });
        })
        .response
    };
    if !clickable {
        return draw(ui);
    }
    // Sensed below the buttons, so a click on a button stays the button's.
    let strip = ui.scope_builder(UiBuilder::new().sense(Sense::click()), draw);
    strip.response.on_hover_cursor(CursorIcon::PointingHand)
}

/// `show` as a strip under the toolbar, in a top panel named `id`.
pub fn top(ctx: &egui::Context, id: &'static str, b: Banner, buttons: impl FnOnce(&mut Ui)) -> Response {
    egui::TopBottomPanel::top(id).frame(egui::Frame::new()).show(ctx, |ui| show(ui, b, buttons)).inner
}
