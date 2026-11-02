//! Preferences > Appearance > Theme: the "Custom theme files" disclosure's header, a Regular
//! quiet target (chevron and label) hung left so its chevron lines up with the theme cards. It
//! stays open while a theme file is in use; otherwise a click opens or closes it, remembered in
//! egui memory for the session only.

use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::chevron;
use crate::ui::widgets::quiet_target::{self, State};
use eframe::egui::{Align2, CornerRadius, Rect, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

const SIZE: ControlSize = ControlSize::Regular;
/// The chevron's box, and the gap between it and the label.
const CHEVRON_BOX: f32 = 16.0;
const CHEVRON_GAP: f32 = 6.0;

/// The header labelled `label`; whether the disclosure is open this frame. `pinned` (a theme
/// file is in use) keeps it open and ignores clicks, saying why in the `pinned_tip`.
pub fn header(ui: &mut Ui, label: &str, pinned: bool, pinned_tip: &str) -> bool {
    let id = ui.make_persistent_id("theme_files_open");
    let m = SIZE.metrics();
    let font = type_scale::font(type_scale::BODY);
    let text_w = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), color::text()).size().x);
    let w = m.ghost_pad_x + CHEVRON_BOX + CHEVRON_GAP + text_w + m.quiet_pad_x;
    // Laid out at the cards' left edge, the target hangs `ghost_pad_x` left of it.
    let (slot, _) = ui.allocate_exact_size(vec2(w - m.ghost_pad_x, m.h), Sense::hover());
    let rect = Rect::from_min_max(pos2(slot.left() - m.ghost_pad_x, slot.top()), slot.max);
    let r = ui.interact(rect, id.with("header"), Sense::click());
    let mut user_open = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    if r.clicked() && !pinned {
        user_open = !user_open;
        ui.data_mut(|d| d.insert_temp(id, user_open));
    }
    let open = pinned || user_open;
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    r.ctx.accesskit_node_builder(r.id, |n| n.set_expanded(open));
    let p = ui.painter();
    quiet_target::paint(p, rect, CornerRadius::same(m.radius), SIZE, State::of(&r, false));
    let chevron_box = Rect::from_min_size(pos2(slot.left(), rect.top()), vec2(CHEVRON_BOX, m.h));
    chevron(p, chevron_box.center(), open, color::weak());
    p.text(pos2(chevron_box.right() + CHEVRON_GAP, rect.center().y), Align2::LEFT_CENTER, label, font, color::text());
    if pinned {
        r.on_hover_text(pinned_tip);
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    /// The expanded state the header labelled "Files" reports, `pinned` or not, on a first frame.
    fn reported(pinned: bool) -> Option<bool> {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| header(ui, "Files", pinned, "Pinned"));
        });
        let update = out.platform_output.accesskit_update.expect("AccessKit is on");
        update.nodes.into_iter().find(|(_, n)| n.label() == Some("Files")).and_then(|(_, n)| n.is_expanded())
    }

    #[test]
    fn the_header_reports_whether_it_is_open() {
        assert_eq!(reported(false), Some(false), "closed until clicked");
        assert_eq!(reported(true), Some(true), "open while a theme file is in use");
    }
}
