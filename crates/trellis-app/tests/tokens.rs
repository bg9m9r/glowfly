//! Keeps views on the design tokens (`ui/theme/tokens.rs`). Views name a corner
//! (`radius::MD`, `radius::pill(h)`), an elevation (`tokens::shadow`) and a focus ring
//! (`tokens::focus_ring`, `focus_ring_inset`, `focus_halo`) instead of drawing their own, so no
//! file under `src/ui/` may:
//!
//! - put a number straight into `CornerRadius::same(` or `.corner_radius(`, or after `nw: ` in a
//!   `CornerRadius { … }`
//! - declare a `*RADIUS*` constant as a number
//! - build a `Shadow { … }` or set `popup_shadow` (outside `theme/mod.rs`)
//! - stroke a rect in the accent colour (a hand-made focus ring)
//!
//! `tokens.rs` itself is exempt, and so are the icon shapes (`ICON_FILES`), whose corners are
//! geometry, not the corner scale. `ALLOWED` names the few deliberate exceptions. Test code is
//! skipped, as in `type_scale.rs`.

#[path = "support/views.rs"]
mod views;

use views::{code_lines, view_sources};

/// The file that defines the tokens.
const TOKENS_FILE: &str = "ui/theme/tokens.rs";
/// The file that installs egui's style, popup shadow included.
const STYLE_FILE: &str = "ui/theme/mod.rs";

/// Files of icon and illustration shapes.
const ICON_FILES: &[&str] = &["ui/widgets/icon.rs", "ui/icons.rs", "ui/icons_panel.rs", "ui/empty_state_icons.rs", "ui/graph/marks.rs"];

/// (file, text on the line, reason) for each deliberate exception. Each must still match.
const ALLOWED: &[(&str, &str, &str)] = &[
    ("ui/tour.rs", "rect_stroke(t.shrink(1.0)", "the tour's spotlight outline, not a focus ring"),
    ("ui/terminal/paint.rs", "CursorStyle::Block => painter.rect_stroke", "the terminal's block cursor, not a focus ring"),
    ("ui/welcome_cards.rs", "CornerRadius::same(2), s, StrokeKind::Middle", "a corner of the painted folder glyph"),
];

fn starts_with_digit(s: &str) -> bool {
    s.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// The name of a `const` declared on `line` and the text after its `=`, if any.
fn const_decl(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start();
    let rest =
        rest.strip_prefix("pub(super) ").or_else(|| rest.strip_prefix("pub(crate) ")).or_else(|| rest.strip_prefix("pub ")).unwrap_or(rest);
    let rest = rest.strip_prefix("const ")?;
    let (name, after) = rest.split_once(':')?;
    let (_, value) = after.split_once('=')?;
    Some((name.trim(), value))
}

/// Problems on one line of code in the file at `rel`, as messages.
fn line_problems(rel: &str, line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for call in ["CornerRadius::same(", ".corner_radius("] {
        if line.match_indices(call).any(|(i, _)| starts_with_digit(&line[i + call.len()..])) {
            found.push(format!("a number in `{call}`; use a radius token (XS, SM, MD, LG, XL, pill)"));
        }
    }
    if let Some(i) = line.find("CornerRadius {")
        && let Some(j) = line[i..].find("nw: ")
        && starts_with_digit(&line[i + j + 4..])
    {
        found.push("a number in `CornerRadius { nw: … }`; use radius::top, left or right".to_owned());
    }
    if let Some((name, value)) = const_decl(line)
        && name.contains("RADIUS")
        && starts_with_digit(value)
    {
        found.push(format!("`{name}` is a number; set it to a radius token"));
    }
    if line.contains("Shadow {") || (line.contains("popup_shadow =") && rel != STYLE_FILE) {
        found.push("a hand-made shadow; use tokens::shadow(Elevation::…)".to_owned());
    }
    if line.contains("rect_stroke(") && line.contains("accent") {
        found.push("an accent stroke; focus rings go through tokens::focus_ring / focus_ring_inset".to_owned());
    }
    found
}

/// Problems in the file at `rel` (under `src/`), as "line: message", with the lines `ALLOWED`
/// lets through counted in `used`.
fn violations(rel: &str, src: &str, used: &mut Vec<usize>) -> Vec<String> {
    if rel == TOKENS_FILE || ICON_FILES.contains(&rel) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        let problems = line_problems(rel, line);
        if problems.is_empty() {
            continue;
        }
        if let Some(k) = ALLOWED.iter().position(|(f, pat, _)| *f == rel && line.contains(pat)) {
            used.push(k);
            continue;
        }
        found.extend(problems.into_iter().map(|m| format!("{}: {m}", i + 1)));
    }
    found
}

#[test]
fn views_use_the_design_tokens() {
    let sources = view_sources();
    assert!(sources.iter().any(|(rel, _)| rel == TOKENS_FILE), "{TOKENS_FILE} is still a view file");
    for f in ICON_FILES {
        assert!(sources.iter().any(|(rel, _)| rel == f), "{f} is still a view file");
    }
    let mut used = Vec::new();
    let mut problems = Vec::new();
    for (rel, text) in &sources {
        problems.extend(violations(rel, text, &mut used).into_iter().map(|v| format!("{rel}:{v}")));
    }
    assert!(problems.is_empty(), "off the design tokens:\n{}", problems.join("\n"));
    for (k, (file, pat, why)) in ALLOWED.iter().enumerate() {
        assert!(used.contains(&k), "ALLOWED entry matches nothing: {file} `{pat}` ({why})");
    }
}

#[test]
fn the_check_catches_literal_radii_shadows_and_rings() {
    let src = "const CHIP_RADIUS: u8 = 6;\nconst ROW_H: f32 = 22.0;\nfn a(p: &Painter) {\n    p.rect_filled(r, CornerRadius::same(4), c);\n    Frame::new().corner_radius(8);\n    let t = CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 };\n    let s = Shadow { offset: [0, 4], blur: 16, spread: 0, color: c };\n    p.rect_stroke(r, 4, Stroke::new(2.0, color::accent()), StrokeKind::Outside);\n    p.rect_filled(r, CornerRadius::same(radius::SM), c);\n}\n";
    let found = violations("ui/op_strip.rs", src, &mut Vec::new());
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').next().unwrap()).collect();
    assert_eq!(lines, ["1", "4", "5", "6", "7", "8"], "{found:?}");
    assert!(violations(TOKENS_FILE, src, &mut Vec::new()).is_empty());
    assert!(violations("ui/widgets/icon.rs", src, &mut Vec::new()).is_empty());
    assert!(violations("ui/op_strip.rs", "// CornerRadius::same(4) in a comment\n", &mut Vec::new()).is_empty());
}

#[test]
fn the_style_file_may_set_the_popup_shadow() {
    let src = "fn a(v: &mut Visuals) {\n    v.popup_shadow = shadow(Elevation::Float, p);\n}\n";
    assert!(violations(STYLE_FILE, src, &mut Vec::new()).is_empty());
    assert_eq!(violations("ui/op_strip.rs", src, &mut Vec::new()).len(), 1);
}

#[test]
fn allowed_entries_let_their_line_through() {
    let src = "fn a(p: &Painter) {\n    p.rect_stroke(t.shrink(1.0), CornerRadius::same(radius::MD), Stroke::new(2.0_f32, color::accent()), StrokeKind::Inside);\n}\n";
    let mut used = Vec::new();
    assert!(violations("ui/tour.rs", src, &mut used).is_empty());
    assert_eq!(used, [0]);
    assert_eq!(violations("ui/other.rs", src, &mut Vec::new()).len(), 1);
}
