//! Keeps every hover, press and selection on the one state ladder (control rule R8): a view
//! lights a row through `widgets::row_fill` and a target through `widgets::quiet_target` (both
//! end in `button_look::paint_target` or `row_fill`, the only places a state becomes a fill), so
//! no file under `src/` may `rect_filled` with `color::hover()` or `color::selected()` itself.
//!
//! The helpers and the theme are exempt (`HOMES`). `LEFT` counts, per file, the fills not moved
//! onto the ladder yet; it only shrinks: a file with fewer than its count fails until the count
//! is lowered, and one with more fails outright. Test code is skipped (`support/views.rs`).

use crate::support::views;

use views::{code_lines, view_sources};

/// The files that turn a state into a fill.
const HOMES: &[&str] = &["widgets/row_fill.rs", "widgets/button_look.rs"];

/// The folder of the palette and tokens, which defines the fills.
const THEME: &str = "theme/";

/// (file, how many state fills it still paints itself, why). Only shrinks.
const LEFT: &[(&str, usize, &str)] = &[
    ("blame/body.rs", 2, "the blame band's selected and hovered commit"),
    ("conflict_band.rs", 1, "a conflict band's hovered pick"),
    ("conflict_fold.rs", 2, "the conflict view's fold bar and its button"),
    ("diff/fold.rs", 3, "the diff's fold bar and its two buttons"),
    ("diff/thread_marks.rs", 1, "a hovered comment-thread marker"),
    ("forge_filter_menu.rs", 2, "a facet chip and a hovered menu row"),
    ("history_file.rs", 1, "the file history's selected row"),
    ("issue/picker.rs", 1, "the issue picker's clear cross"),
    ("panel_rails.rs", 1, "a folded panel's edge strip"),
    ("repo_row.rs", 1, "a repository row in a menu, which keeps the menu's own hover"),
    ("terminal/tabs.rs", 1, "a terminal tab's hover"),
    ("trust_chip.rs", 1, "the trust chip's hover"),
];

/// The state colours a view may not fill with itself.
const STATE_FILLS: &[&str] = &["color::hover()", "color::selected()"];

/// The lines (1-based) of `src` that fill a rect with a state colour.
fn state_fills(src: &str) -> Vec<usize> {
    code_lines(src)
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("rect_filled(") && STATE_FILLS.iter().any(|c| line.contains(c)))
        .map(|(i, _)| i + 1)
        .collect()
}

/// The problems across `sources` against the allowlist `left`, as messages.
fn problems(sources: &[(String, String)], left: &[(&str, usize, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    for (rel, text) in sources {
        if HOMES.contains(&rel.as_str()) || rel.starts_with(THEME) {
            continue;
        }
        let lines = state_fills(text);
        let allowed = left.iter().find(|(f, _, _)| f == rel).map_or(0, |(_, n, _)| *n);
        if lines.len() > allowed {
            let at: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            out.push(format!(
                "src/{rel}: {} state fill(s) at line {} (allowed {allowed}); use row_fill or quiet_target",
                lines.len(),
                at.join(", ")
            ));
        } else if lines.len() < allowed {
            out.push(format!("src/{rel}: {} state fill(s), fewer than its LEFT count {allowed}; lower it", lines.len()));
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
fn views_light_their_states_through_the_shared_ladder() {
    let found = problems(&view_sources(), LEFT);
    assert!(found.is_empty(), "state fills off the ladder:\n{}", found.join("\n"));
}

#[test]
fn the_check_catches_state_fills_and_holds_the_allowlist() {
    let src = "fn a(p: &Painter, r: &Response) {\n    p.rect_filled(rect, 0.0, color::hover());\n    \
               p.rect_filled(rect, radius::SM, if on { color::selected() } else { color::panel() });\n    \
               let (rect, _) = ui.allocate_exact_size(v, Sense::hover());\n    p.rect_filled(rect, 0.0, color::panel());\n    \
               // p.rect_filled(rect, 0.0, color::hover());\n}\n";
    assert_eq!(state_fills(src), [2, 3]);
    let one = |rel: &str| vec![(rel.to_owned(), src.to_owned())];
    assert_eq!(problems(&one("op_strip.rs"), &[]).len(), 1, "a view's own fills fail");
    assert!(problems(&one("widgets/row_fill.rs"), &[]).is_empty(), "the helpers are exempt");
    assert!(problems(&one("theme/surfaces.rs"), &[]).is_empty(), "the theme is exempt");
    assert!(problems(&one("op_strip.rs"), &[("op_strip.rs", 2, "")]).is_empty(), "the counted fills pass");
    assert_eq!(problems(&one("op_strip.rs"), &[("op_strip.rs", 3, "")]).len(), 1, "a count above the fills fails: lower it");
    assert_eq!(problems(&one("op_strip.rs"), &[("gone.rs", 2, "")]).len(), 2, "a stale entry fails");
}
