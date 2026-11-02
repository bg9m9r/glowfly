//! Tests for `chip.rs`: the two sizes' heights, widths and padding, the least width of a badge,
//! centring in a fixed rect, and cutting to a widest.

use super::*;
use eframe::egui::{self, Context};

fn in_frame(f: impl FnOnce(&mut Ui)) {
    let ctx = Context::default();
    let mut f = Some(f);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| (f.take().unwrap())(ui));
    });
}

#[test]
fn every_chip_is_one_of_two_heights() {
    in_frame(|ui| {
        for (size, h) in [(ChipSize::Badge, 18.0), (ChipSize::Chip, 22.0)] {
            assert_eq!(Chip::new(size, "main").show(ui).rect.height(), h);
            assert_eq!(rect_at(size, 0.0, 40.0, 30.0).height(), h);
        }
        assert_eq!(quiet_chip(ui, "a1b2c3d", "SHA").rect.height(), 22.0);
    });
}

#[test]
fn a_chip_is_its_text_and_padding() {
    in_frame(|ui| {
        let text_w = ui.painter().layout_no_wrap("feature/x".into(), type_scale::font(type_scale::SMALL), Color32::WHITE).size().x;
        let w = Chip::new(ChipSize::Chip, "feature/x").width(ui.ctx());
        assert!((w - (text_w + 16.0)).abs() < 0.01, "{w} for text {text_w}");
    });
}

#[test]
fn a_badge_is_at_least_a_circle() {
    assert_eq!(width(ChipSize::Badge, 0.0), 18.0);
    assert_eq!(width(ChipSize::Badge, 4.0), 18.0);
    assert_eq!(width(ChipSize::Badge, 20.0), 32.0);
    assert_eq!(width(ChipSize::Chip, 0.0), 16.0);
}

#[test]
fn content_centres_only_off_its_natural_width() {
    let r = Rect::from_min_size(pos2(10.0, 0.0), vec2(18.0, 18.0));
    // A letter narrower or wider than the square's room centres on it.
    assert_eq!(content_left(r, ChipSize::Badge, 4.0), 17.0);
    assert_eq!(content_left(r, ChipSize::Badge, 8.0), 15.0);
    // At its natural width the text sits on the padding.
    let natural = Rect::from_min_size(pos2(10.0, 0.0), vec2(32.0, 18.0));
    assert_eq!(content_left(natural, ChipSize::Badge, 20.0), 16.0);
}

#[test]
fn a_chip_sits_on_a_whole_point_centred_on_its_line() {
    let r = rect_at(ChipSize::Badge, 5.0, 20.5, 30.0);
    assert_eq!(r.top(), 11.0);
    assert_eq!(rect_at(ChipSize::Chip, 5.0, 20.0, 30.0).center().y, 20.0);
}

#[test]
fn a_chip_past_its_widest_is_cut() {
    in_frame(|ui| {
        let long = "a-very-long-branch-name-that-does-not-fit";
        let w = Chip::new(ChipSize::Chip, long).max_w(80.0).width(ui.ctx());
        assert!(w <= 80.0 + 0.5, "{w}");
        assert!(Chip::new(ChipSize::Chip, long).width(ui.ctx()) > 80.0);
    });
}

#[test]
fn a_lead_takes_the_glyph_slot_and_gap() {
    in_frame(|ui| {
        let plain = Chip::new(ChipSize::Chip, "Running").width(ui.ctx());
        let dotted = Chip::new(ChipSize::Chip, "Running").lead(Lead::Dot(Color32::RED)).width(ui.ctx());
        assert_eq!(dotted - plain, 16.0);
    });
}
