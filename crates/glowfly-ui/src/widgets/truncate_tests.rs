//! The truncate helpers' tests: cut galleys, middle, left and path cuts, and counted labels.

use super::*;
use crate::ui::theme::type_scale;
use eframe::egui::text::TextFormat;
use eframe::egui::{Align, LayerId};

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    ctx
}

fn font() -> FontId {
    type_scale::font(type_scale::BODY)
}

/// The characters a galley draws, the "…" included.
fn drawn(g: &Galley) -> String {
    g.rows.iter().flat_map(|r| r.glyphs.iter().map(|gl| gl.chr)).collect()
}

const LONG: &str = "Move the settings loader into its own module so the preferences window, the command line and the \
                    tests share one place to read it";

#[test]
fn short_text_is_not_cut() {
    let ctx = ctx();
    let g = cut_galley(&ctx, cut_job("Short", font(), Color32::WHITE), 200.0);
    assert!(!g.elided);
    assert_eq!(drawn(&g), "Short");
}

#[test]
fn long_text_ends_on_whole_glyphs_and_an_ellipsis() {
    let ctx = ctx();
    assert!(LONG.chars().count() >= 110, "{}", LONG.len());
    let g = cut_galley(&ctx, cut_job(LONG, font(), Color32::WHITE), 80.0);
    assert!(g.elided);
    assert!(g.size().x <= 80.0, "{}", g.size().x);
    let shown = drawn(&g);
    let kept = shown.strip_suffix(ELLIPSIS).expect("ends with an ellipsis");
    assert!(!kept.is_empty() && LONG.starts_with(kept), "{shown:?}");
    assert_eq!(g.text(), LONG, "the galley keeps the whole text");
}

#[test]
fn text_exactly_as_wide_as_the_room_is_not_cut() {
    let ctx = ctx();
    let natural = ctx.fonts(|f| f.layout_no_wrap("Fix build on Windows".into(), font(), Color32::WHITE)).size().x;
    let g = cut_galley(&ctx, cut_job("Fix build on Windows", font(), Color32::WHITE), natural);
    assert!(!g.elided);
    assert_eq!(drawn(&g), "Fix build on Windows");
}

#[test]
fn a_cut_inside_the_second_section_keeps_the_first_whole() {
    let ctx = ctx();
    let mut job = LayoutJob::default();
    let summary = "Add app skeleton";
    job.append(summary, 0.0, TextFormat { valign: Align::Center, ..TextFormat::simple(font(), Color32::WHITE) });
    job.append(LONG, 10.0, TextFormat { valign: Align::Center, ..TextFormat::simple(type_scale::font(type_scale::SMALL), Color32::GRAY) });
    let summary_w = ctx.fonts(|f| f.layout_no_wrap(summary.into(), font(), Color32::WHITE)).size().x;
    let g = cut_galley(&ctx, job, summary_w + 120.0);
    assert!(g.elided);
    assert!(drawn(&g).starts_with(summary), "{:?}", drawn(&g));
}

#[test]
fn too_narrow_or_nan_paints_nothing_and_counts_as_cut() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
    let at = pos2(10.0, 20.0);
    for w in [3.0, f32::NAN, -5.0] {
        let c = paint_cut_text(&p, at, "Search results page", font(), Color32::WHITE, w);
        assert!(c.cut, "{w}");
        assert_eq!(c.rect.width(), 0.0, "{w}");
    }
    assert!(!paint_cut_text(&p, at, "", font(), Color32::WHITE, 3.0).cut, "empty text is never cut");
    let wide = paint_cut_text(&p, at, "Short", font(), Color32::WHITE, 200.0);
    assert!(!wide.cut && wide.rect.width() > 0.0);
}

#[test]
fn only_a_cut_line_is_hovered() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(50.0, 16.0));
    let inside = Some(pos2(10.0, 8.0));
    assert!(!Cut { rect, cut: false }.hovered(inside));
    assert!(Cut { rect, cut: true }.hovered(inside));
    assert!(!Cut { rect, cut: true }.hovered(Some(pos2(80.0, 8.0))));
    assert!(!Cut { rect, cut: true }.hovered(None));
}

/// Runs `f` once in a central panel `width` points wide.
pub(crate) fn in_panel(width: f32, f: impl FnOnce(&mut Ui)) {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(width, 300.0));
    let mut f = Some(f);
    let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(f) = f.take() {
                f(ui);
            }
        });
    });
    assert!(f.is_none(), "the panel ran");
}

#[test]
fn a_label_in_a_wrapping_row_moves_to_the_next_row_instead_of_being_cut() {
    in_panel(300.0, |ui| {
        ui.horizontal_wrapped(|ui| {
            let row_top = ui.cursor().top();
            ui.add_space(ui.available_width() - 30.0);
            let (r, cut) = cut_label(ui, "3 days ago", f32::INFINITY);
            assert!(!cut, "a whole row has room for it");
            assert!(r.rect.top() > row_top, "it moved onto the next row: {:?}", r.rect);
        });
    });
}

#[test]
fn a_label_in_a_plain_row_is_cut_to_the_room_left() {
    in_panel(300.0, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() - 30.0);
            let (r, cut) = cut_label(ui, "3 days ago", f32::INFINITY);
            assert!(cut);
            assert!(r.rect.width() <= 30.0, "{:?}", r.rect);
        });
    });
}

#[test]
fn a_middle_cut_keeps_both_ends() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
    let f = font();
    assert_eq!(cut_middle_in(&p, "main", &f, 200.0), "main", "short text is kept");
    let name = "feature/accessibility-pass-for-dialogs";
    let cut = cut_middle_in(&p, name, &f, 160.0);
    let w = p.layout_no_wrap(cut.clone(), f.clone(), Color32::WHITE).size().x;
    assert!(w <= 160.0, "{cut:?} is {w} wide");
    assert_eq!(cut.matches(ELLIPSIS).count(), 1, "{cut:?}");
    let (head, tail) = cut.split_once(ELLIPSIS).unwrap();
    assert!(!head.is_empty() && name.starts_with(head), "{cut:?}");
    assert!(!tail.is_empty() && name.ends_with(tail), "{cut:?}");
    assert!(tail.chars().count() + 1 >= head.chars().count(), "{cut:?}");
    assert_eq!(cut_middle_in(&p, name, &f, 4.0), ELLIPSIS.to_string(), "too narrow");
}

#[test]
fn a_middle_cut_never_splits_a_character() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
    let name = "fonctionnalité/écran-de-connexion-réécrit-über-alles";
    for w in [40.0, 80.0, 120.0, 160.0] {
        let cut = cut_middle_in(&p, name, &font(), w);
        let (head, tail) = cut.split_once(ELLIPSIS).expect("cut");
        assert!(name.starts_with(head) && name.ends_with(tail), "{w}: {cut:?}");
    }
}

#[test]
fn a_path_cut_keeps_whole_folders() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
    let f = font();
    let w = |s: &str| p.layout_no_wrap(s.to_owned(), f.clone(), Color32::WHITE).size().x;
    let path = r"C:\work\clients\northwind\platform\services";
    let bs = '\u{5c}';
    assert_eq!(cut_path_in(&p, path, &f, 1000.0, bs), path, "a path that fits is kept");
    assert_eq!(cut_path_in(&p, path, &f, w(r"C:\…\platform\services") + 1.0, bs), r"C:\…\platform\services");
    assert_eq!(cut_path_in(&p, path, &f, w(r"C:\…\services") + 1.0, bs), r"C:\…\services");
    let (share, server) = (format!("{bs}{bs}server{bs}share{bs}team{bs}api"), format!("{bs}{bs}server{bs}…{bs}api"));
    assert_eq!(cut_path_in(&p, &share, &f, w(&server) + 1.0, bs), server, "a share keeps its server");
    assert_eq!(cut_path_in(&p, "/home/me/src/app/lib", &f, w("/home/…/lib") + 1.0, '/'), "/home/…/lib");
    // Too narrow for whole folders: cut in the middle.
    let cut = cut_path_in(&p, path, &f, w(r"C:\…\services") - 4.0, bs);
    assert_eq!(cut.matches(ELLIPSIS).count(), 1, "{cut:?}");
    assert!(w(&cut) <= w(r"C:\…\services") - 4.0, "{cut:?}");
}

#[test]
fn full_tip_puts_the_whole_text_first_when_cut() {
    assert_eq!(full_tip(true, "name", Some("path".into())).as_deref(), Some("name\npath"));
    assert_eq!(full_tip(true, "name", None).as_deref(), Some("name"));
    assert_eq!(full_tip(false, "name", Some("path".into())).as_deref(), Some("path"));
    assert_eq!(full_tip(false, "name", None), None);
}

#[test]
fn a_count_splits_off_the_end_of_a_title() {
    assert_eq!(split_count("Staged files (3)"), ("Staged files", " (3)"));
    assert_eq!(split_count("Nicht bereitgestellt (12)"), ("Nicht bereitgestellt", " (12)"));
    assert_eq!(split_count("Commit"), ("Commit", ""), "no count");
    assert_eq!(split_count("Files (all)"), ("Files (all)", ""), "not a number");
    assert_eq!(split_count("Files ()"), ("Files ()", ""));
    assert_eq!(split_count("(3)"), ("(3)", ""), "a count alone has no label to cut");
}

#[test]
fn a_counted_label_is_cut_before_its_count() {
    let ctx = ctx();
    let text = "Nicht bereitgestellt (1)";
    let (label, count) = counted_galleys(&ctx, text, font(), Color32::WHITE, 100.0);
    assert!(label.elided);
    assert_eq!(drawn(&count), " (1)");
    assert!(drawn(&label).starts_with("Nicht") && drawn(&label).ends_with(ELLIPSIS), "{:?}", drawn(&label));
    assert!(label.size().x + count.size().x <= 100.0);
    let (whole, _) = counted_galleys(&ctx, text, font(), Color32::WHITE, 400.0);
    assert!(!whole.elided && drawn(&whole) == "Nicht bereitgestellt");
}
