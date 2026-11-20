//! Animations pace themselves (`widgets/redraw.rs`): no view asks for the next frame straight
//! away with a bare `request_repaint()` except the interaction-driven ones in `LEFT`, and no
//! view draws egui's own spinner (which repaints every frame); spinners are `widgets::Spinner`.
//! `LEFT` only shrinks: a file with fewer bare requests than its count fails until the count is
//! lowered, and one with more fails outright. Comments, literals and test code are skipped.

use crate::support::lexer::bare_lines;
use crate::support::views::view_sources;

/// The one file that draws the arc itself.
const SPINNER: &str = "widgets/spinner.rs";

/// (file, how many bare `request_repaint()` calls it still makes, why). Only shrinks.
const LEFT: &[(&str, usize, &str)] = &[
    ("diff/text_sel.rs", 1, "drag-selecting past the edge keeps scrolling while the pointer is held"),
    ("getting_started.rs", 1, "the cards' measured height settles"),
    ("graph/mod.rs", 1, "the hovered branch's rows show on the next frame"),
    ("list_focus.rs", 2, "focus moved by a key lands on the next frame"),
    ("menu_bar/bar.rs", 1, "a menu opened from the keyboard draws open"),
    ("menu_bar/mod.rs", 1, "a menu opened from the keyboard draws open"),
    ("menu_keys/level.rs", 5, "a menu's keyboard steps"),
    ("menu_keys/mod.rs", 1, "a menu's keyboard steps"),
    ("prefs/reveal.rs", 1, "the reveal flash fades out within a second"),
    ("terminal/mouse.rs", 1, "drag-scrolling past the edge"),
    ("toolbar.rs", 1, "a measured width settles"),
    ("toolbar_pickers.rs", 1, "a measured width settles"),
    ("widgets/menu_submenu.rs", 4, "a submenu opening or closing"),
    ("widgets/menu_submenu_keyed.rs", 1, "a submenu opening or closing"),
    ("widgets/tooltip_motion.rs", 1, "a tooltip rising into place"),
];

/// The lines (1-based) of `src` that ask for a frame at once.
fn bare_repaints(src: &str) -> Vec<usize> {
    bare_lines(src).iter().enumerate().filter(|(_, l)| l.contains("request_repaint()")).map(|(i, _)| i + 1).collect()
}

/// The lines (1-based) of `src` that use egui's spinner: `egui::Spinner`, `ui.spinner()`, or `Spinner::new(` not
/// reached through `widgets::`.
fn egui_spinners(src: &str) -> Vec<usize> {
    let ours = |l: &str| l.contains("widgets::Spinner::new(") || l.contains("super::Spinner::new(");
    bare_lines(src)
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("egui::Spinner") || l.contains(".spinner()") || (l.contains("Spinner::new(") && !ours(l)))
        .map(|(i, _)| i + 1)
        .collect()
}

fn at(lines: &[usize]) -> String {
    lines.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

/// The problems across `sources` against the allowlist `left`, as messages.
fn problems(sources: &[(String, String)], left: &[(&str, usize, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    for (rel, text) in sources {
        if rel == SPINNER {
            continue;
        }
        let spinners = egui_spinners(text);
        if !spinners.is_empty() {
            out.push(format!("src/{rel}: egui's spinner at line {}; use widgets::Spinner", at(&spinners)));
        }
        let lines = bare_repaints(text);
        let allowed = left.iter().find(|(f, _, _)| f == rel).map_or(0, |(_, n, _)| *n);
        if lines.len() > allowed {
            out.push(format!(
                "src/{rel}: {} bare request_repaint() at line {} (allowed {allowed}); pace an animation with widgets::redraw",
                lines.len(),
                at(&lines)
            ));
        } else if lines.len() < allowed {
            out.push(format!("src/{rel}: {} bare request_repaint(), fewer than its LEFT count {allowed}; lower it", lines.len()));
        }
    }
    for (file, _, _) in left {
        if !sources.iter().any(|(rel, _)| rel == file) {
            out.push(format!("LEFT names src/{file}, which is gone; remove its entry"));
        }
    }
    out
}

#[test]
fn animations_pace_themselves() {
    let found = problems(&view_sources(), LEFT);
    assert!(found.is_empty(), "animations off the shared pace:\n{}", found.join("\n"));
}

#[test]
fn the_check_catches_bare_repaints_and_egui_spinners() {
    let src = "fn a(ui: &mut Ui) {\n    ui.ctx().request_repaint();\n    // ui.ctx().request_repaint();\n    \
               let s = \"request_repaint()\";\n    ui.ctx().request_repaint_after(REDRAW);\n}\n";
    assert_eq!(bare_repaints(src), [2], "a comment, a literal and the delayed request pass");
    let spinners = "fn b(ui: &mut Ui) {\n    ui.add(egui::Spinner::new());\n    ui.add(Spinner::new());\n    \
                    ui.add(crate::widgets::Spinner::new());\n    ui.add(widgets::Spinner::new());\n}\n";
    assert_eq!(egui_spinners(spinners), [2, 3], "egui's spinner fails, the widgets one passes");
    let one = |rel: &str, text: &str| vec![(rel.to_owned(), text.to_owned())];
    assert_eq!(problems(&one("op_strip.rs", src), &[]).len(), 1, "a bare request fails");
    assert_eq!(problems(&one("op_strip.rs", spinners), &[]).len(), 1, "an egui spinner fails");
    assert!(problems(&one("op_strip.rs", src), &[("op_strip.rs", 1, "")]).is_empty(), "the counted request passes");
    assert!(problems(&one(SPINNER, spinners), &[]).is_empty(), "the spinner's own file is exempt");
    let over = problems(&one("op_strip.rs", src), &[("op_strip.rs", 2, "")]);
    assert!(over.len() == 1 && over[0].contains("lower it"), "a count above the calls fails: {over:?}");
    assert_eq!(problems(&one("op_strip.rs", src), &[("op_strip.rs", 1, ""), ("gone.rs", 1, "")]).len(), 1, "a stale entry fails");
}
