//! The sidebar's and the file lists' row geometry: heights and the right-hand inset.

use super::*;
use crate::ui::widgets::section_header;

/// Runs one pass (after one that installs the bundled fonts) with `body` in a 300 × 400 panel;
/// returns the pass's shapes.
fn pass(mut body: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0))), ..Default::default() };
    fonts::install(&ctx);
    let _ = ctx.run(input(), |_| {});
    let out = ctx.run(input(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            body(ui);
        });
    });
    out.shapes
}

#[test]
fn list_rows_and_section_headers_have_their_heights() {
    pass(|ui| {
        let r = list_row(ui, ROW_INDENT, "main", Some(("2", Color32::GRAY)), false, true);
        assert_eq!(r.rect.height(), LIST_ROW_H);
        let (r, marker, _) = list_row_cut(ui, ROW_INDENT, "feature", None, 18.0, false, false);
        assert_eq!(r.rect.height(), LIST_ROW_H);
        assert_eq!(marker.right(), r.rect.right() - PAD_X, "the marker slot ends PAD_X from the edge");
        assert_eq!(marker.width(), 18.0);
        let mut open = true;
        let h = section_header(ui, "LOCAL", Some(3), &mut open);
        assert_eq!(h.rect.height(), SECTION_H);
    });
}

#[test]
fn right_hand_parts_end_pad_x_from_the_edge() {
    let mut row = Rect::NOTHING;
    let shapes = pass(|ui| {
        row = list_row(ui, ROW_INDENT, "main", Some(("origin", Color32::GRAY)), false, false).rect;
    });
    let ends: Vec<f32> = shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "origin" => Some(t.pos.x + t.galley.size().x),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 1, "the right-hand text is painted once");
    assert!((ends[0] - (row.right() - PAD_X)).abs() < 0.01, "ends at {}, row right {}", ends[0], row.right());
}

#[test]
fn a_hidden_row_takes_a_rows_height() {
    pass(|ui| {
        egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(1000.0);
            let before = ui.cursor().min.y;
            assert!(list_row_hidden(ui), "a row far below the view is skipped");
            assert_eq!(ui.cursor().min.y - before, LIST_ROW_H);
        });
    });
}

#[test]
fn file_rows_are_a_fixed_height_with_their_contents_centred() {
    pass(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let mut rows = Vec::new();
        egui::ScrollArea::vertical().show_rows(ui, LIST_ROW_H, 5, |ui, range| {
            for i in range {
                let r = file_row(ui, |ui| {
                    let b = ui.small_button("Stage").rect;
                    let l = ui.selectable_label(false, egui::RichText::new(format!("src/file_{i}.rs")).monospace()).rect;
                    (b, l)
                });
                rows.push((r.response.rect, r.inner));
            }
        });
        assert_eq!(rows.len(), 5);
        for (row, (button, label)) in &rows {
            assert_eq!(row.height(), LIST_ROW_H);
            for (what, r) in [("button", button), ("label", label)] {
                assert!((r.center().y - row.center().y).abs() <= 0.5, "{what} centred: {} in {}", r.center().y, row.center().y);
            }
        }
        // `show_rows` places row i at i × (LIST_ROW_H + item spacing), as the rows are drawn.
        let step = rows[1].0.top() - rows[0].0.top();
        assert_eq!(step, LIST_ROW_H + 4.0);
    });
}
