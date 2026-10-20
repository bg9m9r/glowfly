use super::*;
use crate::ui::access;
use egui_kittest::Harness;
use std::cell::RefCell;
use std::rc::Rc;

/// What one pass saw: the probe's widgets, its keyboard focus and the widgets' own rects.
type Seen = (Vec<Value>, Value, Vec<Rect>);

#[test]
fn named_widgets_are_listed_with_their_roles_rects_and_the_keyboard_focus() {
    let seen: Rc<RefCell<Seen>> = Rc::default();
    let out = seen.clone();
    let mut text = String::new();
    let mut h = Harness::new_ui(move |ui| {
        let (_, b) = ui.allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::click());
        access::name_button(&b, true, "Painted");
        let l = ui.label("Hello");
        let f = ui.add(egui::TextEdit::singleline(&mut text).hint_text("Type here"));
        access::name_field(&f, "Search text");
        f.request_focus();
        let ctx = ui.ctx();
        *out.borrow_mut() = (widgets_in(ctx, &panels(ctx)), keyboard_focus(ctx), vec![b.rect, l.rect, f.rect]);
    });
    h.run();
    let (widgets, focus, rects) = seen.borrow().clone();
    let find = |name: &str| {
        widgets
            .iter()
            .find(|w| w["label"] == name || w["value"] == name)
            .unwrap_or_else(|| panic!("no widget {name:?} in {widgets:#?}"))
            .clone()
    };
    for ((name, role), rect) in [("Painted", "Button"), ("Hello", "Label"), ("Search text", "TextInput")].into_iter().zip(rects) {
        let w = find(name);
        assert_eq!(w["role"], role, "{name}");
        // The probe's rect is egui's widget rect: the response's, or around it with the
        // field's frame, with the same centre.
        let r = w["rect"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect::<Vec<_>>();
        let got = Rect::from_min_max(Pos2::new(r[0], r[1]), Pos2::new(r[2], r[3]));
        assert!(got.contains_rect(rect) && (got.center() - rect.center()).length() < 0.01, "{name}: {got:?} vs {rect:?}");
        if role != "TextInput" {
            assert_eq!(got, rect, "{name}");
        }
        assert_eq!(w["region"], "central", "{name}");
        assert_eq!(w["enabled"], true, "{name}");
    }
    assert_eq!(find("Hello")["value"], "Hello", "a label's text is its value");
    assert_eq!(focus["label"], "Search text");
    assert_eq!(focus["role"], "TextInput");
}

#[test]
fn regions_come_from_the_panels_drawn_this_pass() {
    let seen: Rc<RefCell<Vec<(&'static str, Rect)>>> = Rc::default();
    let out = seen.clone();
    let mut h = Harness::new(move |ctx| {
        // A panel egui remembers but that isn't drawn this pass is not a region.
        let stale = PanelState { rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(2000.0, 2000.0)) };
        ctx.data_mut(|d| d.insert_persisted(Id::new("toolbar"), stale));
        egui::SidePanel::left("refs").exact_width(200.0).show(ctx, |ui| ui.label("left"));
        egui::SidePanel::right("detail").exact_width(300.0).show(ctx, |ui| ui.label("right"));
        egui::CentralPanel::default().show(ctx, |ui| ui.label("middle"));
        *out.borrow_mut() = panels(ctx);
    });
    h.run();
    let panels = seen.borrow().clone();
    let rect = |id: &str| panels.iter().find(|(p, _)| *p == id).map(|(_, r)| *r).unwrap_or_else(|| panic!("no {id} in {panels:?}"));
    let (refs, detail) = (rect("refs"), rect("detail"));
    assert!(panels.iter().all(|(id, _)| *id != "toolbar"), "{panels:?}");
    assert_eq!(region_of(&panels, None, refs.center()), "sidebar");
    assert_eq!(region_of(&panels, None, detail.center()), "details");
    let middle = Pos2::new((refs.right() + detail.left()) / 2.0, refs.center().y);
    assert_eq!(region_of(&panels, None, middle), "central");
    let palette = Rect::from_center_size(middle, egui::vec2(50.0, 50.0));
    assert_eq!(region_of(&panels, Some(palette), middle), "palette");
}
