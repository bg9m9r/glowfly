//! Not a snapshot check: with `GLOWFLY_SHEET_DIR` set, renders every built-in theme into that
//! folder for judging its colours by eye (docs/COLOUR.md, "No brown"): the main window over the
//! pinned-branch graph (`<id>_main.png`) and a strip of its ref chips on the background, a
//! selected and a hovered row, then its bands, tints and washes (`<id>_chips.png`).
//! `GLOWFLY_SHEET_DIR=<dir> cargo test -p glowfly colour_sheet -- --ignored`.

use super::with_bands;
use crate::state::Selection;
use crate::ui::graph::paint_box;
use crate::ui::snapshot_tests::{harness_palette_sized, remove_fixture};
use crate::ui::theme::branch_colours::{chip_in, line_in, swatches};
use crate::ui::theme::contrast::{self, ChipStyle};
use crate::ui::theme::surfaces::Tone;
use crate::ui::theme::{self, apply_style, builtin, color, color::Palette, egui_theme};
use eframe::egui::{self, Color32, FontId, Painter, Rect, pos2, vec2};
use glowfly_core::branch_colour::Swatch;
use glowfly_core::repo;
use glowfly_core::settings::BuiltinTheme;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const W: f32 = 1500.0;
const ROW_H: f32 = 34.0;

/// Draws one ref chip from `x` centred on `y` as the graph does; returns its width.
fn chip(p: &Painter, x: f32, y: f32, text: &str, look: contrast::ChipLook) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), FontId::proportional(theme::type_scale::SMALL), look.text);
    let w = g.size().x + 14.0;
    paint_box(p, Rect::from_min_size(pos2(x, y - 11.0), vec2(w, 22.0)), look);
    p.galley(pos2(x + 7.0, y - g.size().y / 2.0), g, look.text);
    w
}

/// A row of every chip a palette draws, centred on `y`.
fn chips(p: &Painter, pal: &Palette, y: f32) {
    let mut x = 12.0;
    let mut row: Vec<(ChipStyle, Swatch, String)> =
        vec![(ChipStyle::Head, Swatch::Main, "HEAD".into()), (ChipStyle::CurrentBranch, Swatch::Main, "✔ main".into())];
    for s in swatches().skip(1) {
        let Swatch::Slot(i) = s else { continue };
        row.push((ChipStyle::Branch, s, format!("{:?}", pal.branch.names[usize::from(i)]).to_lowercase()));
    }
    row.push((ChipStyle::CurrentBranch, Swatch::Slot(3), "✔ orange".into()));
    row.push((ChipStyle::Remote, Swatch::Main, "origin/main".into()));
    row.push((ChipStyle::Tag, Swatch::Main, "v1.2.0".into()));
    for (style, s, text) in row {
        x += chip(p, x, y, &text, contrast::ref_chip(style, chip_in(s, pal), pal)) + 5.0;
    }
}

/// The strip for one palette.
fn strip(pal: Palette, name: &str) -> image::RgbaImage {
    let name = name.to_owned();
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(W, 6.0 * ROW_H + 70.0)).build_ui(move |ui| {
        color::set_palette(&pal);
        apply_style(ui.ctx(), &pal);
        ui.ctx().set_theme(egui_theme(&pal));
        let p = ui.painter();
        let full = ui.max_rect();
        p.rect_filled(full, 0.0, pal.bg);
        let row = |i: f32| Rect::from_min_size(pos2(full.left(), full.top() + i * ROW_H), vec2(W, ROW_H));
        p.text(row(0.0).left_center() + vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, &name, FontId::proportional(14.0), pal.text);
        chips(p, &pal, row(1.0).center().y);
        for (i, selected) in [(2.0, true), (3.0, false)] {
            crate::ui::widgets::row_fill::paint(p, row(i), selected, !selected);
            chips(p, &pal, row(i).center().y);
        }
        // Bands (and highlighted bands) per lane, then the tints and washes.
        let mut x = 12.0;
        let r4 = row(4.0);
        for s in swatches() {
            let lane = line_in(s, &pal);
            let band = crate::ui::graph::band_color(lane, &pal);
            p.rect_filled(Rect::from_min_size(pos2(x, r4.top() + 3.0), vec2(40.0, 13.0)), 0.0, band);
            p.rect_filled(
                Rect::from_min_size(pos2(x, r4.top() + 16.0), vec2(40.0, 13.0)),
                0.0,
                crate::ui::graph::highlight_color(band, lane),
            );
            p.circle_filled(pos2(x + 46.0, r4.center().y), 4.0, lane);
            x += 58.0;
        }
        let mut swatch: Vec<Color32> =
            vec![pal.selected, pal.hover, pal.tool_backing(), pal.tool_pressed(), pal.attention.banner, pal.tag_bg];
        swatch.extend(Tone::ALL.iter().map(|t| pal.tone_tint(*t).0));
        swatch.extend([pal.status.modified, pal.attention.warn, pal.accent, pal.status.info].iter().map(|c| pal.wash_on(pal.bg, *c, 0.18)));
        swatch.extend([pal.diff_add, pal.diff_del, pal.diff_add_sel, pal.diff_del_sel, pal.hunk_bg]);
        let r5 = row(5.0);
        for (i, c) in swatch.into_iter().enumerate() {
            p.rect_filled(Rect::from_min_size(pos2(12.0 + i as f32 * 62.0, r5.top() + 3.0), vec2(56.0, ROW_H + 30.0)), 4.0, c);
        }
    });
    h.run();
    h.render().expect("render")
}

/// The main window of `t` over the pinned-branch graph, a commit on main selected.
fn main_window(t: BuiltinTheme) -> image::RgbaImage {
    let (mut st, dir) = with_bands(&format!("sheet-{}", t.id()));
    let Selection::Commit(oid) = st.session.selection else { panic!("a commit is selected") };
    st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    st.session.reads.line_counts.start_commit(oid);
    st.session.reads.line_counts.finish_commit(oid, Arc::new(repo::commit_line_counts(&dir, oid).unwrap()));
    let mut h = harness_palette_sized(st, *builtin(t), vec2(1600.0, 820.0));
    h.run_steps(4);
    let img = h.render().expect("render");
    drop(h);
    remove_fixture(&dir);
    img
}

fn save(img: &image::RgbaImage, dir: &Path, name: &str) {
    img.save(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
}

#[test]
#[ignore = "renders the colour sheet for review; needs GLOWFLY_SHEET_DIR"]
fn colour_sheet() {
    let dir = PathBuf::from(std::env::var("GLOWFLY_SHEET_DIR").expect("GLOWFLY_SHEET_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for t in BuiltinTheme::ALL {
        save(&strip(*builtin(t), t.label()), &dir, &format!("{}_chips.png", t.id()));
        save(&main_window(t), &dir, &format!("{}_main.png", t.id()));
    }
}
