//! A dialog's header (`dialog_frame`): the title (with its optional painted icon) and the weak
//! subtitle under it, and the close button at the row's right end. The button is a painted
//! cross in a 28 px box, weak at rest and on a hover fill when pointed at. It is not focusable
//! (Esc is the keyboard's way out), and a click on it is kept for `take_closed`, which the
//! caller reads every frame after showing the dialog.

use crate::ui::access;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{Icon, paint_icon};
use eframe::egui::{self, Color32, CornerRadius, Id, LayerId, Rect, RichText, Sense, TextStyle, Ui, pos2, vec2};
use trellis_core::i18n::Lang;

/// The close button's box.
const CLOSE: f32 = 28.0;
/// Between the title and the close button.
const TITLE_GAP: f32 = 12.0;
/// The icon before the title, and its gap.
const ICON: f32 = 18.0;
const ICON_W: f32 = 26.0;
/// How far the close box reaches into the dialog's padding, up and right (optical alignment).
const NUDGE: f32 = 4.0;

/// The no-wrap width the header needs: the title row (icon, title, gap, close button) or the
/// subtitle, whichever is wider.
pub(super) fn need(ui: &Ui, title: &str, subtitle: Option<&str>, icon: bool) -> f32 {
    let title_font = type_scale::strong(ui, type_scale::SUBHEAD);
    let body = TextStyle::Body.resolve(ui.style());
    let w = |text: &str, font: egui::FontId| ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER).size().x);
    let row = if icon { ICON_W } else { 0.0 } + w(title, title_font) + TITLE_GAP + CLOSE;
    subtitle.map_or(row, |s| row.max(w(s, body)))
}

/// Draws the header at the cursor, the close button against `right_edge`. Returns whether the
/// close button was clicked.
pub(super) fn draw(ui: &mut Ui, t: Lang, title: &str, subtitle: Option<&str>, icon: Option<Icon>, right_edge: f32) -> bool {
    let top = ui.cursor().top();
    let title_w = (ui.available_width() - CLOSE - TITLE_GAP).max(40.0);
    let text = RichText::new(title).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::strong());
    ui.allocate_ui_with_layout(vec2(title_w, 0.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.set_max_width(title_w);
        if let Some(icon) = icon {
            let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
            paint_icon(ui.painter(), rect.center(), icon, color::text());
        }
        ui.add(egui::Label::new(text).wrap());
    });
    if let Some(s) = subtitle {
        ui.add_space(4.0 - ui.spacing().item_spacing.y);
        ui.add(egui::Label::new(RichText::new(s).color(color::weak())).wrap());
    }
    close_button(ui, t, Rect::from_min_size(pos2(right_edge - CLOSE + NUDGE, top - NUDGE), vec2(CLOSE, CLOSE)))
}

/// The painted close button in `rect`; true when clicked.
fn close_button(ui: &mut Ui, t: Lang, rect: Rect) -> bool {
    let tip = t.tr("dialog.close.tip");
    let r = ui.interact(rect, ui.id().with("dialog-close"), Sense::CLICK);
    access::name_button(&r, true, tip);
    let col = if r.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), color::hover());
        color::text()
    } else {
        color::weak()
    };
    paint_icon(ui.painter(), rect.center(), Icon::Close, col);
    let clicked = r.clicked();
    r.on_hover_text(tip);
    if clicked {
        ui.data_mut(|d| d.insert_temp(closed_id(ui.layer_id()), true));
    }
    clicked
}

/// Where a click on the close button of the dialog on `layer` is kept until it is taken.
fn closed_id(layer: LayerId) -> Id {
    Id::new("trellis-dialog-closed").with(layer)
}

/// Whether the close button of the dialog `resp` showed was clicked this frame. Takes the
/// click, so call it every frame after showing the dialog: a stale one never reaches the next.
pub fn take_closed<T>(resp: &egui::ModalResponse<T>) -> bool {
    let id = closed_id(resp.response.layer_id);
    resp.response.ctx.data_mut(|d| d.remove_temp::<bool>(id)).unwrap_or(false)
}
