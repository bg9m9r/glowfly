//! Keeps view text on the type scale (`theme/type_scale.rs`). Views name a step
//! (`type_scale::CAPTION`, `SMALL`, `BODY`, `SUBHEAD`, `TITLE`) instead of a pixel size, so no
//! file under `src/` may:
//!
//! - put a number straight into `FontId::proportional(`, `FontId::monospace(`, `FontId::new(`,
//!   `semibold(ui, `, `.text_size(` or `.size(` (a spinner's `.size(` is not text and is fine)
//! - declare a font-size constant (`*FONT*`, `*_PX`, `*_SIZE`) as a number
//! - use egui's 9 px `TextStyle::Small` or `RichText::small()`
//! - use `RichText::strong()`, which only changes the colour; headers use `type_scale::section`
//!   and emphasis `type_scale::strong`
//! - colour a `Link` by hand: every link uses the accent colour
//!
//! `type_scale.rs` itself is exempt, and `ALLOWED` names the few deliberate exceptions. Test
//! code is skipped, as in `controls.rs`.

use crate::support::views;

use views::{code_lines, view_sources};

/// The file that defines the scale.
const SCALE_FILE: &str = "theme/type_scale.rs";

/// A deliberate exception: (file, text on the line, reason).
type Allowed = (&'static str, &'static str, &'static str);

/// The deliberate exceptions. Each must still match. None today: icons are drawn with lines.
const ALLOWED: &[Allowed] = &[];

/// Calls that must take a type-scale step, not a number.
const SIZED_CALLS: &[&str] = &["FontId::proportional(", "FontId::monospace(", "FontId::new(", "semibold(ui, ", ".text_size(", ".size("];

fn starts_with_digit(s: &str) -> bool {
    s.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// The name of a `const` declared on `line` and the text after its `=`, if any.
fn const_decl(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("pub(crate) ").or_else(|| rest.strip_prefix("pub ")).unwrap_or(rest);
    let rest = rest.strip_prefix("const ")?;
    let (name, after) = rest.split_once(':')?;
    let (_, value) = after.split_once('=')?;
    Some((name.trim(), value))
}

fn is_font_const(name: &str) -> bool {
    name.contains("FONT") || name.ends_with("_PX") || name.ends_with("_SIZE")
}

/// Problems on one line of code, as messages.
fn line_problems(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for call in SIZED_CALLS {
        if *call == ".size(" && line.contains("Spinner") {
            continue;
        }
        if line.match_indices(call).any(|(i, _)| starts_with_digit(&line[i + call.len()..])) {
            found.push(format!("a number in `{call}`; use a type_scale step (CAPTION, SMALL, BODY, SUBHEAD or TITLE)"));
        }
    }
    if let Some((name, value)) = const_decl(line)
        && is_font_const(name)
        && starts_with_digit(value)
    {
        found.push(format!("`{name}` is a number; set it to a type_scale step"));
    }
    if line.contains("TextStyle::Small") || (line.contains("RichText") && line.contains(".small()")) {
        found.push("egui's 9 px small text; use `.size(type_scale::SMALL)` or CAPTION".to_owned());
    }
    if line.contains(".strong()") {
        found.push("`.strong()` only changes the colour; use type_scale::section for a header or type_scale::strong".to_owned());
    }
    if line.contains("Link::new(") && line.contains(".color(") {
        found.push("a link coloured by hand; links use the accent colour".to_owned());
    }
    found
}

/// Problems in the file at `rel` (under `src/`), as "line: message", with the lines `allowed`
/// lets through counted in `used`.
fn violations_with(rel: &str, src: &str, allowed: &[Allowed], used: &mut Vec<usize>) -> Vec<String> {
    if rel == SCALE_FILE {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        if let Some(k) = allowed.iter().position(|(f, pat, _)| *f == rel && line.contains(pat)) {
            used.push(k);
            continue;
        }
        found.extend(line_problems(line).into_iter().map(|m| format!("{}: {m}", i + 1)));
    }
    found
}

/// `violations_with` the `ALLOWED` exceptions.
fn violations(rel: &str, src: &str, used: &mut Vec<usize>) -> Vec<String> {
    violations_with(rel, src, ALLOWED, used)
}

#[test]
fn view_text_uses_the_type_scale() {
    let sources = view_sources();
    assert!(sources.iter().any(|(rel, _)| rel == SCALE_FILE), "{SCALE_FILE} is still a view file");
    let mut used = Vec::new();
    let mut problems = Vec::new();
    for (rel, text) in &sources {
        problems.extend(violations(rel, text, &mut used).into_iter().map(|v| format!("{rel}:{v}")));
    }
    assert!(problems.is_empty(), "text off the type scale:\n{}", problems.join("\n"));
    for (k, (file, pat, why)) in ALLOWED.iter().enumerate() {
        assert!(used.contains(&k), "ALLOWED entry matches nothing: {file} `{pat}` ({why})");
    }
}

#[test]
fn the_check_catches_literal_sizes() {
    let src = "fn a(ui: &mut Ui) {\n    ui.label(RichText::new(t).size(12.0));\n    let f = FontId::proportional(13.0);\n    ui.add(Spinner::new().size(14.0));\n    let g = FontId::monospace(type_scale::SMALL);\n}\n";
    let found = violations("op_strip.rs", src, &mut Vec::new());
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with("2: a number in `.size(`"), "{found:?}");
    assert!(found[1].starts_with("3: a number in `FontId::proportional(`"), "{found:?}");
    assert!(violations(SCALE_FILE, src, &mut Vec::new()).is_empty());
}

#[test]
fn the_check_catches_font_constants_small_strong_and_coloured_links() {
    let src = "const TITLE_FONT: f32 = 10.5;\npub const PATH_PX: f32 = type_scale::BODY;\nconst ROW_H: f32 = 22.0;\nfn a(ui: &mut Ui) {\n    ui.label(RichText::new(t).small());\n    ui.label(RichText::new(t).strong());\n    ui.add(Link::new(t).color(c));\n    let s = TextStyle::Small;\n}\n";
    let found = violations("op_strip.rs", src, &mut Vec::new());
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').next().unwrap()).collect();
    assert_eq!(lines, ["1", "5", "6", "7", "8"], "{found:?}");
    assert!(violations("op_strip.rs", "// .strong() in a comment\n", &mut Vec::new()).is_empty());
}

#[test]
fn allowed_entries_let_their_line_through() {
    let src = "fn a(p: &Painter) {\n    p.text(c, Align2::CENTER_CENTER, \"{ }\", FontId::monospace(13.0), col);\n}\n";
    let allowed: &[Allowed] = &[("widgets/icon.rs", "\"{ }\", FontId::monospace(13.0)", "a glyph sized to its icon")];
    let mut used = Vec::new();
    assert!(violations_with("widgets/icon.rs", src, allowed, &mut used).is_empty());
    assert_eq!(used, [0]);
    assert_eq!(violations_with("other.rs", src, allowed, &mut Vec::new()).len(), 1);
}
