//! Keeps views on the design tokens (`theme/tokens.rs`). Views name a corner
//! (`radius::MD`, `radius::pill(h)`), an elevation (`tokens::shadow`) and a focus ring
//! (`tokens::focus_ring`, `focus_ring_inset`, `focus_halo`) instead of drawing their own, so no
//! file under `src/` may:
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
//!
//! The size check (`theme/sizes.rs`, control sizing spec §7) finds sizes views set by hand
//! outside `widgets/` and `theme/`: a button's `.height(`, `.min_height(` or `.text_size(`, a
//! text field's `.margin(`, a `*_H`, `*HEIGHT`, `*PAD`, `*PAD_X` or `CHIP_*` const set to a
//! number, `allocate_exact_size(vec2(…, <number>))`, and text centred by hand
//! (`center().y - <galley>.size().y / 2.0`). It only logs them for now, against the counted
//! allowlist `SIZE_LITERALS`, which only shrinks; it starts failing once the sizes are swept.

use crate::support::views;

use views::{code_lines, view_sources};

/// The file that defines the tokens.
const TOKENS_FILE: &str = "theme/tokens.rs";
/// The file that installs egui's style, popup shadow included.
const STYLE_FILE: &str = "theme/mod.rs";

/// Files of icon and illustration shapes.
const ICON_FILES: &[&str] = &["widgets/icon.rs", "icons.rs", "icons_panel.rs", "empty_state_icons.rs", "graph/marks.rs"];

/// (file, text on the line, reason) for each deliberate exception. Each must still match.
const ALLOWED: &[(&str, &str, &str)] = &[
    ("tour.rs", "rect_stroke(t.shrink(1.0)", "the tour's spotlight outline, not a focus ring"),
    ("welcome_cards.rs", "CornerRadius::same(2), s, StrokeKind::Middle", "a corner of the painted folder glyph"),
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
    let found = violations("op_strip.rs", src, &mut Vec::new());
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').next().unwrap()).collect();
    assert_eq!(lines, ["1", "4", "5", "6", "7", "8"], "{found:?}");
    assert!(violations(TOKENS_FILE, src, &mut Vec::new()).is_empty());
    assert!(violations("widgets/icon.rs", src, &mut Vec::new()).is_empty());
    assert!(violations("op_strip.rs", "// CornerRadius::same(4) in a comment\n", &mut Vec::new()).is_empty());
}

#[test]
fn the_style_file_may_set_the_popup_shadow() {
    let src = "fn a(v: &mut Visuals) {\n    v.popup_shadow = shadow(Elevation::Float, p);\n}\n";
    assert!(violations(STYLE_FILE, src, &mut Vec::new()).is_empty());
    assert_eq!(violations("op_strip.rs", src, &mut Vec::new()).len(), 1);
}

#[test]
fn allowed_entries_let_their_line_through() {
    let src = "fn a(p: &Painter) {\n    p.rect_stroke(t.shrink(1.0), CornerRadius::same(radius::MD), Stroke::new(2.0_f32, color::accent()), StrokeKind::Inside);\n}\n";
    let mut used = Vec::new();
    assert!(violations("tour.rs", src, &mut used).is_empty());
    assert_eq!(used, [0]);
    assert_eq!(violations("other.rs", src, &mut Vec::new()).len(), 1);
}

/// How many hand-set sizes the size check finds today. Lower it as call sites take a
/// `ControlSize`; never raise it.
const SIZE_LITERALS: usize = 157;

/// The folders that define sizes, which the size check skips.
const SIZE_HOMES: &[&str] = &["widgets/", "theme/"];

/// Whether `s` starts with a number, after an optional minus sign.
fn starts_with_number(s: &str) -> bool {
    starts_with_digit(s.trim_start().trim_start_matches('-'))
}

/// Whether `line` calls `call` with an argument (not `rect.height()`).
fn calls_with_argument(line: &str, call: &str) -> bool {
    line.match_indices(call).any(|(i, _)| !line[i + call.len()..].starts_with(')'))
}

/// The hand-set sizes on one line of code, as messages.
fn size_problems(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let button = line.contains("buttons::") || line.contains("button") || line.contains("styled");
    for call in [".height(", ".min_height(", ".text_size("] {
        if button && calls_with_argument(line, call) {
            found.push(format!("a button's `{call}`; give it a ControlSize"));
        }
    }
    if line.contains(".margin(") {
        found.push("a text field's `.margin(`; fields take their padding from their ControlSize".to_owned());
    }
    if let Some((name, value)) = const_decl(line)
        && line.split_once(':').and_then(|(_, rest)| rest.split_once('=')).is_some_and(|(ty, _)| ty.trim() == "f32")
        && (name.ends_with("_H")
            || name.ends_with("HEIGHT")
            || name.ends_with("PAD")
            || name.ends_with("PAD_X")
            || name.starts_with("CHIP_"))
        && starts_with_number(value)
    {
        found.push(format!("`{name}` is a number; take it from theme::sizes"));
    }
    for call in ["allocate_exact_size(vec2(", "allocate_exact_size(egui::vec2("] {
        for (i, _) in line.match_indices(call) {
            let args = &line[i + call.len()..];
            // The second argument of vec2: after the first top-level comma.
            let mut depth = 0;
            let second = args.char_indices().find_map(|(j, c)| match c {
                '(' => {
                    depth += 1;
                    None
                }
                ')' if depth == 0 => Some(None),
                ')' => {
                    depth -= 1;
                    None
                }
                ',' if depth == 0 => Some(Some(&args[j + 1..])),
                _ => None,
            });
            if let Some(Some(h)) = second
                && starts_with_number(h)
            {
                found.push("`allocate_exact_size` with a number for the height; take it from a size".to_owned());
            }
        }
    }
    if let Some(i) = line.find("center().y - ")
        && line[i..].contains(".size().y / 2.0")
    {
        found.push("text centred by hand; use sizes::text_pos".to_owned());
    }
    found
}

/// The hand-set sizes in the file at `rel`, as "line: message".
fn size_violations(rel: &str, src: &str) -> Vec<String> {
    if SIZE_HOMES.iter().any(|home| rel.starts_with(home)) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        found.extend(size_problems(line).into_iter().map(|m| format!("{}: {m}", i + 1)));
    }
    found
}

/// Log-only for now: lists the hand-set sizes and compares their count with `SIZE_LITERALS`,
/// without failing.
#[test]
fn views_take_their_sizes_from_the_size_tokens() {
    let mut found = Vec::new();
    for (rel, text) in &view_sources() {
        found.extend(size_violations(rel, text).into_iter().map(|v| format!("{rel}:{v}")));
    }
    eprintln!("size check (log only): {} hand-set sizes, allowlist {SIZE_LITERALS}\n{}", found.len(), found.join("\n"));
    if found.len() != SIZE_LITERALS {
        eprintln!("size check: the count moved from {SIZE_LITERALS} to {}; lower SIZE_LITERALS when it falls", found.len());
    }
}

#[test]
fn the_size_check_catches_each_hand_set_size() {
    let src = "const ROW_H: f32 = 22.0;
const CHIP_GAP: f32 = 4.0;
const SIDE_PAD: f32 = -2.0;
const TOP_H: f32 = ROW_H;
const ROWS_H: usize = 3;
fn a(ui: &mut Ui, g: &Galley) {
    ui.add(buttons::quiet(t).height(20.0));
    let b = button.min_height(BUTTON_H);
    let s = styled.text_size(body);
    TextEdit::singleline(&mut s).margin(m);
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let (r, _) = ui.allocate_exact_size(vec2(f(1.0, 2.0), h), Sense::hover());
    let y = rect.center().y - g.size().y / 2.0;
    let h = rect.height();
    ui.set_min_height(20.0);
}
";
    let found = size_violations("op_strip.rs", src);
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').next().unwrap()).collect();
    assert_eq!(lines, ["1", "2", "3", "7", "8", "9", "10", "11", "13"], "{found:?}");
    assert!(size_violations("widgets/buttons.rs", src).is_empty(), "widgets/ defines sizes");
    assert!(size_violations("theme/sizes.rs", src).is_empty(), "theme/ defines sizes");
}
