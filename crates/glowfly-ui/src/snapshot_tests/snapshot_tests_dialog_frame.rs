//! The three button styles and their disabled look in a dark, a light and a high-contrast theme,
//! and the frame's options for a dialog with a look of its own: a header the caller draws with
//! the shared close box, a set width and a least body height.

use super::dialog_frame::{self, Cx, Dialog, Footer};
use super::snapshot_check::CheckSnapshot;
use super::theme::color::{self, Palette};
use super::theme::{self, apply_style, egui_theme};
use super::widgets::buttons;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Id, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;

fn render_buttons(name: &str, p: Palette) {
    let mut first = true;
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(440.0, 150.0)).build_ui(move |ui| {
        color::set_palette(&p);
        apply_style(ui.ctx(), &p);
        ui.ctx().set_theme(egui_theme(&p));
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
        ui.add_space(8.0);
        for enabled in [true, false] {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(buttons::primary("Create branch").enabled(enabled));
                ui.add(buttons::secondary("Cancel").enabled(enabled));
                ui.add(buttons::danger("Delete branch").enabled(enabled));
            });
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let r = ui.add(buttons::primary("Focused").min_width(76.0));
            if first {
                r.request_focus();
                first = false;
            }
            ui.add(buttons::secondary("Browse…").small());
        });
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn button_styles() {
    render_buttons("button_styles_dark", theme::DARK);
    render_buttons("button_styles_light", theme::LIGHT);
}

/// A dialog with its own header and close box, 500 px wide with a 200 px least body and only a
/// Close button; the state counts the frames `take_closed` reported a click.
fn own_header(size: Vec2) -> Harness<'static, u32> {
    crate::ui::snapshot_check::builder().with_size(size).build_state(
        |ctx, closes: &mut u32| {
            color::set_palette(&theme::GARDEN_DARK);
            apply_style(ctx, &theme::GARDEN_DARK);
            ctx.set_theme(egui_theme(&theme::GARDEN_DARK));
            let id = Id::new("own-header");
            let cx = Cx { t: Lang::En, keyboard: true, key: id.with("dialog") };
            let resp = theme::show_modal(ctx, theme::dialog(ctx, id), |ui| {
                let header = |ui: &mut egui::Ui| {
                    ui.label("Own header");
                    let (right, top) = (ui.cursor().left() + ui.available_width(), ui.cursor().top());
                    dialog_frame::close_button(ui, "Close own", right, top);
                };
                let mut d = Dialog::new(cx, "Unused title").header(header).width(500.0).min_body(200.0).begin(ui);
                d.body(ui, |_| {});
                d.finish(ui, Footer::new().close());
            });
            if dialog_frame::take_closed(&resp) {
                *closes += 1;
            }
        },
        0,
    )
}

#[test]
fn dialog_own_header() {
    let mut h = own_header(vec2(800.0, 700.0));
    h.run();
    assert!(h.query_by_label(Lang::En.tr("dialog.close.tip")).is_none(), "no standard header");
    let header = h.get_by_label("Own header").bounding_box().unwrap();
    let close = h.get_by_role_and_label(Role::Button, "Close").bounding_box().unwrap();
    assert!(close.y0 - header.y1 >= 200.0, "the body keeps its least height: {header:?} {close:?}");
    h.get_by_label("Close own").click();
    h.run();
    assert_eq!(*h.state(), 1, "the close box's click reaches take_closed");
    h.run();
    assert_eq!(*h.state(), 1, "and is taken once");
    drop(h);
    // A narrow window clamps the set width.
    let mut h = own_header(vec2(400.0, 700.0));
    h.run();
    let close = h.get_by_role_and_label(Role::Button, "Close").bounding_box().unwrap();
    assert!(close.x0 >= 0.0 && close.x1 <= 400.0, "inside the window: {close:?}");
}
