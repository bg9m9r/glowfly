//! The size check (`theme/sizes.rs`, control sizing spec §7): views outside `widgets/` and
//! `theme/` take their sizes from `theme::sizes`. The hand-set sizes still left (a button's
//! height or text size, a field's margin, a `*_H`/`*PAD` number, a literal
//! `allocate_exact_size` height) are counted against `SIZE_LITERALS`, and the check fails when
//! the count moves either way. Text centred by hand, or by a centre anchor passed to egui's
//! `Painter::text`, fails at any count: `widgets::paint_text` and `sizes::text_pos` set text on a
//! whole point, never low.

use super::{const_decl, starts_with_digit};
use crate::support::lexer::bare_lines;
use crate::support::views::{code_lines, view_sources};

/// How many hand-set sizes the size check finds: the final count after S5. It only shrinks as
/// call sites take a `ControlSize`; never raise it.
const SIZE_LITERALS: usize = 109;

/// The folders that define sizes, which the size check skips.
const SIZE_HOMES: &[&str] = &["widgets/", "theme/"];

/// The anchors that centre text vertically.
const CENTRE_ANCHORS: &[&str] = &["Align2::LEFT_CENTER", "Align2::RIGHT_CENTER", "Align2::CENTER_CENTER"];

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
    found
}

/// The start of the expression that ends at byte `end` of `line`: a run of names, dots and
/// call parentheses (`piece.galley`, `ui.fonts()`).
fn expr_start(line: &str, end: usize) -> usize {
    line[..end].rfind(|c: char| !(c.is_ascii_alphanumeric() || "_.()".contains(c))).map_or(0, |i| i + 1)
}

/// Text centred by hand on one line of code, as messages: half a galley's height (or size)
/// taken from a centre line (`cy - g.size().y / 2.0`, `mid - 0.5 * g.size().y`,
/// `rect.center() - g.size() / 2.0`). Half a height added to a top (`top + g.size().y / 2.0`,
/// a centre line) is fine.
fn centring_problems(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for half in [".size().y / 2.0", ".size() / 2.0"] {
        for (i, _) in line.match_indices(half) {
            if line[..expr_start(line, i)].ends_with(" - ") {
                found.push("text centred by hand; use sizes::text_pos or sizes::centred_on".to_owned());
            }
        }
    }
    for (i, _) in line.match_indices(".size().y") {
        if line[..expr_start(line, i)].ends_with("- 0.5 * ") {
            found.push("text centred by hand; use sizes::text_pos or sizes::centred_on".to_owned());
        }
    }
    found
}

/// The 0-based lines of `bare` (code with literal text blanked) where a `Painter::text`-style
/// `.text(` call is given a centre anchor. The call may span lines; it is reported at its
/// `.text(` line. Parentheses are counted on bare code, so those in a string don't count.
fn centred_painter_text(bare: &[String]) -> Vec<usize> {
    let joined = bare.join("\n");
    let mut found = Vec::new();
    for (i, _) in joined.match_indices(".text(") {
        let open = i + ".text(".len();
        if joined[open..].starts_with(')') {
            continue;
        }
        let mut depth = 1;
        let close = joined[open..].char_indices().find_map(|(j, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + j)
        });
        let args = &joined[open..close.unwrap_or(joined.len())];
        if CENTRE_ANCHORS.iter().any(|a| args.contains(a)) {
            found.push(joined[..i].matches('\n').count());
        }
    }
    found
}

/// The size problems in the file at `rel`, as "line: message": the counted hand-set sizes, and
/// the text centred by hand (never allowed). Files in `SIZE_HOMES` are skipped.
fn size_violations(rel: &str, src: &str) -> (Vec<String>, Vec<String>) {
    if SIZE_HOMES.iter().any(|home| rel.starts_with(home)) {
        return (Vec::new(), Vec::new());
    }
    let (mut counted, mut centring) = (Vec::new(), Vec::new());
    for (i, line) in code_lines(src).iter().enumerate() {
        counted.extend(size_problems(line).into_iter().map(|m| format!("{}: {m}", i + 1)));
        centring.extend(centring_problems(line).into_iter().map(|m| format!("{}: {m}", i + 1)));
    }
    for i in centred_painter_text(&bare_lines(src)) {
        centring.push(format!("{}: text centred by egui's Painter::text; use widgets::paint_text or sizes::text_pos", i + 1));
    }
    (counted, centring)
}

/// Whether the hand-set sizes `found` match the allowlist count `allowed`: more is a new
/// hand-set size, fewer means the count should come down.
fn size_budget(found: &[String], allowed: usize) -> Result<(), String> {
    let n = found.len();
    if n > allowed {
        Err(format!(
            "{n} hand-set sizes, more than SIZE_LITERALS ({allowed}): take the size from theme::sizes (a ControlSize, its \
             metrics or a row height) instead:\n{}",
            found.join("\n")
        ))
    } else if n < allowed {
        Err(format!("only {n} hand-set sizes now: lower SIZE_LITERALS in tests/view_checks/tokens/size_check.rs to {n}"))
    } else {
        Ok(())
    }
}

#[test]
fn views_take_their_sizes_from_the_size_tokens() {
    let (mut counted, mut centring) = (Vec::new(), Vec::new());
    for (rel, text) in &view_sources() {
        let (c, h) = size_violations(rel, text);
        counted.extend(c.into_iter().map(|v| format!("{rel}:{v}")));
        centring.extend(h.into_iter().map(|v| format!("{rel}:{v}")));
    }
    assert!(centring.is_empty(), "text centred by hand, which sits it up to half a pixel low:\n{}", centring.join("\n"));
    if let Err(e) = size_budget(&counted, SIZE_LITERALS) {
        panic!("{e}");
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
    let y = cy - g.size().y / 2.0;
    let y = mid - 0.5 * g.size().y;
    p.galley(rect.center() - g.size() / 2.0, g, c);
    p.text(at, Align2::LEFT_CENTER, \"Fetch\", f, c);
    p.text(
        pos2(x, y),
        Align2::RIGHT_CENTER,
        \"Push\",
        f,
        c,
    );
    paint_text(p, at, Align2::LEFT_CENTER, \"Fetch\", f, c);
    let line = top + g.size().y / 2.0;
    p.text(at, Align2::LEFT_TOP, \"Fetch\", f, c);
    card.text(t);
    row.text(\"(3)\");
    row.text(\"(3\");
    paint_text(p, at, Align2::CENTER_CENTER, \"Pull\", f, c);
}
";
    let (counted, centring) = size_violations("op_strip.rs", src);
    let lines = |found: &[String]| found.iter().map(|f| f.split(':').next().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(lines(&counted), ["1", "2", "3", "7", "8", "9", "10", "11"], "{counted:?}");
    assert_eq!(lines(&centring), ["13", "16", "17", "18", "19", "20"], "{centring:?}");
    assert_eq!(size_violations("widgets/buttons.rs", src), (Vec::new(), Vec::new()), "widgets/ defines sizes");
    assert_eq!(size_violations("theme/sizes.rs", src), (Vec::new(), Vec::new()), "theme/ defines sizes");
}

#[test]
fn a_planted_literal_fails_the_size_check() {
    let planted =
        |n: usize| (0..n).map(|i| format!("op_strip.rs:{}: `PLANTED_H` is a number; take it from theme::sizes", i + 1)).collect::<Vec<_>>();
    let over = size_budget(&planted(SIZE_LITERALS + 1), SIZE_LITERALS).unwrap_err();
    assert!(over.contains("more than SIZE_LITERALS") && over.contains("PLANTED_H"), "{over}");
    let under = size_budget(&planted(SIZE_LITERALS - 1), SIZE_LITERALS).unwrap_err();
    assert!(under.contains(&format!("lower SIZE_LITERALS in tests/view_checks/tokens/size_check.rs to {}", SIZE_LITERALS - 1)), "{under}");
    assert!(size_budget(&planted(SIZE_LITERALS), SIZE_LITERALS).is_ok());

    let src = "fn a(p: &Painter, c: Pos2) {\n    p.text(c, Align2::CENTER_CENTER, \"3\", f, col);\n}\n";
    let (counted, centring) = size_violations("op_strip.rs", src);
    assert!(counted.is_empty(), "{counted:?}");
    assert_eq!(centring.len(), 1, "a centred Painter::text in a view is reported: {centring:?}");
    assert!(centring[0].starts_with("2: text centred by egui's Painter::text"), "{centring:?}");
    for home in ["widgets/menu.rs", "theme/controls.rs"] {
        assert_eq!(size_violations(home, src), (Vec::new(), Vec::new()), "{home}: centring in the size homes is skipped");
    }
}
