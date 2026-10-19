//! The sidebar's and the file lists' rows: heights, the right-hand inset, the file rows' one
//! click target and their selection's reach.

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
                let name = format!("src/file_{i}.rs");
                let r = file_row(ui, false, &name, |ui| {
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

#[test]
fn a_file_rows_end_part_keeps_its_width_and_the_path_is_cut() {
    let (mut row, mut end, mut path) = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
    pass(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let name = "a/very/long/path/that/never/fits/in/three/hundred/pixels/of/row/width.txt";
        row = file_row_with_end(
            ui,
            false,
            name,
            |ui| end = ui.allocate_exact_size(vec2(60.0, 10.0), Sense::hover()).0,
            |ui| path = ui.label(name).rect,
        )
        .response
        .rect;
    });
    assert_eq!(row.height(), LIST_ROW_H);
    assert_eq!(end.width(), 60.0, "the end part keeps its width");
    assert!((end.right() - row.right()).abs() < 0.01, "flush with the right edge: {end:?} in {row:?}");
    assert!(path.right() <= end.left(), "the path is cut before the end part: {path:?}, {end:?}");
    assert!(path.left() <= row.left() + 0.01, "the path starts at the row's left");
}

/// Runs passes of `body` in a 300 × 400 panel (after one that installs the bundled fonts), one
/// per entry of `frames`, each with that entry's input events.
fn passes(frames: Vec<Vec<egui::Event>>, mut body: impl FnMut(&mut Ui)) {
    let ctx = egui::Context::default();
    let input =
        |events| egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0))), events, ..Default::default() };
    fonts::install(&ctx);
    let _ = ctx.run(input(Vec::new()), |_| {});
    for events in frames {
        let _ = ctx.run(input(events), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                body(ui);
            });
        });
    }
}

/// The frames of a primary click at `at`: lay out, move there, press, release.
fn click_at(at: Pos2) -> Vec<Vec<egui::Event>> {
    let button =
        |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    vec![vec![], vec![egui::Event::PointerMoved(at)], vec![button(true)], vec![button(false)], vec![]]
}

/// A file row holding a Stage button and a path; returns where the row, button and path were
/// laid out and whether the row and the button were clicked, for a click at `at(row, button, path)`.
fn click_file_row(at: impl Fn(Rect, Rect, Rect) -> Pos2) -> (bool, bool) {
    let (mut row, mut button, mut text) = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
    passes(vec![vec![]], |ui| {
        let r = file_row(ui, false, "src/app.rs", |ui| {
            button = ui.small_button("Stage").rect;
            text = row_text(ui, egui::RichText::new("src/app.rs").monospace(), false).rect;
        });
        row = r.response.rect;
    });
    let (mut row_clicked, mut button_clicked) = (false, false);
    passes(click_at(at(row, button, text)), |ui| {
        let r = file_row(ui, false, "src/app.rs", |ui| {
            let b = ui.small_button("Stage");
            row_text(ui, egui::RichText::new("src/app.rs").monospace(), false);
            b.clicked()
        });
        row_clicked |= r.response.clicked();
        button_clicked |= r.inner;
    });
    (row_clicked, button_clicked)
}

#[test]
fn a_click_on_a_file_rows_empty_area_or_text_clicks_the_row() {
    let empty = click_file_row(|row, _, _| pos2(row.right() - 10.0, row.center().y));
    assert_eq!(empty, (true, false), "a click right of the path clicks the row, not the button");
    let text = click_file_row(|_, _, text| text.center());
    assert_eq!(text, (true, false), "a click on the path clicks the row");
}

#[test]
fn a_click_on_a_button_in_a_file_row_clicks_only_the_button() {
    let (row, button) = click_file_row(|_, button, _| button.center());
    assert!(button, "the button takes its own click");
    assert!(!row, "the row is not clicked as well");
}

/// The filled rects among `shapes`, as (rect, colour).
fn fills(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, Color32)> {
    shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) => Some((r.rect, r.fill)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_selected_file_row_paints_across_the_row_and_into_the_padding() {
    color::set_dark(true);
    let (mut sel, mut plain) = (Rect::NOTHING, Rect::NOTHING);
    let shapes = pass(|ui| {
        sel = file_row(ui, true, "src/app.rs", |ui| row_text(ui, egui::RichText::new("src/app.rs"), true)).response.rect;
        plain = file_row(ui, false, "src/lib.rs", |ui| row_text(ui, egui::RichText::new("src/lib.rs"), false)).response.rect;
    });
    let f = fills(&shapes);
    let wide = sel.expand2(vec2(FILE_ROW_BLEED, 0.0));
    assert!(f.contains(&(wide, color::selected())), "the fill reaches FILE_ROW_BLEED past each side: {f:?}");
    let bar = Rect::from_min_size(wide.left_top(), vec2(row_fill::ACCENT_BAR_W, LIST_ROW_H));
    assert!(f.contains(&(bar, color::accent())), "the accent bar sits at the fill's left edge: {f:?}");
    let in_plain = |r: &Rect| r.intersects(plain.shrink(1.0));
    assert!(!f.iter().any(|(r, c)| in_plain(r) && (*c == color::selected() || *c == color::accent() || *c == color::hover())));
}

#[test]
fn row_text_takes_a_selectable_labels_width() {
    color::set_dark(true);
    let (mut text, mut label, mut row) = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
    let shapes = pass(|ui| {
        ui.horizontal(|ui| text = row_text(ui, egui::RichText::new("src/app.rs").monospace(), false).rect);
        ui.horizontal(|ui| label = ui.selectable_label(false, egui::RichText::new("src/app.rs").monospace()).rect);
        row = list_row_cut(ui, ROW_INDENT, "main", None, 0.0, true, false).0.rect;
    });
    assert_eq!(text.width(), label.width(), "unselected rows keep their layout");
    assert_eq!(text.height(), label.height());
    let bar = Rect::from_min_size(row.left_top(), vec2(row_fill::ACCENT_BAR_W, LIST_ROW_H));
    assert!(fills(&shapes).contains(&(bar, color::accent())), "a selected sidebar row has the accent bar");
}
