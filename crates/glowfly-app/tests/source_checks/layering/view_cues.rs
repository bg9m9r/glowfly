//! Views read `AppState::cues` (`ViewCues`: what update and the shell set for them) and never
//! write it: no assignment, `&mut` borrow or mutating method call on a `cues.` path anywhere in
//! glowfly-ui's non-test sources. What views may write is `ViewState` (`st.view`). Until views
//! take `&AppState` and the compiler enforces it (#370), this textual check does.

use crate::lexer::bare_lines;
use crate::views::view_sources;

/// Methods that change what they are called on.
const MUTATING: &[&str] = &[
    "take", "replace", "clear", "push", "pop", "insert", "remove", "retain", "reset", "toggle", "extend", "truncate", "drain", "as_mut",
    "get_mut", "iter_mut", "set",
];

fn is_path(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

/// Whether `line` writes a `cues.` path.
fn writes_cues(line: &str) -> bool {
    line.match_indices("cues.").any(|(at, _)| {
        let before = &line[..at];
        if before.chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            return false; // part of a longer name
        }
        if before.trim_end_matches(is_path).trim_end().ends_with("&mut") {
            return true;
        }
        let after = &line[at + "cues.".len()..];
        let path = &after[..after.find(|c: char| !is_path(c)).unwrap_or(after.len())];
        let rest = after[path.len()..].trim_start();
        if rest.starts_with('(') {
            return path.rsplit('.').next().is_some_and(|m| MUTATING.contains(&m));
        }
        let assign = rest.starts_with('=') && !rest.starts_with("==");
        assign || ["+=", "-=", "*=", "/=", "|=", "&="].iter().any(|op| rest.starts_with(op))
    })
}

#[test]
fn views_never_write_the_cues() {
    let mut report = Vec::new();
    for (rel, text) in view_sources() {
        for (i, line) in bare_lines(&text).iter().enumerate() {
            if writes_cues(line) {
                report.push(format!("crates/glowfly-ui/src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        report.is_empty(),
        "views only read `cues` (ViewCues, set by update and the shell). Push a Msg instead, or move a \
         field the view must write to ViewState (state/view.rs):\n{}",
        report.join("\n")
    );
}

#[test]
fn the_cue_check_catches_writes() {
    for w in [
        "st.cues.now = 5;",
        "st.cues.diff_lines.clear();",
        "let s = &mut st.cues.diff_lines;",
        "let h = std::mem::take(&mut st.cues.hover_tip);",
        "st.cues.window.maximized = true;",
        "st.cues.trunk_near |= 1;",
        "let r = st.cues.file_reveal.take();",
    ] {
        assert!(writes_cues(w), "a write: {w}");
    }
    for r in [
        "let n = st.cues.now;",
        "if st.cues.now == 5 {}",
        "let f = fill(st.cues.backdrop, Surface::Side);",
        "st.cues.op_steps.is_open(snap, step)",
        "let m = &mut st.view.menu_keys; m.focus = st.cues.focused;",
        "let x = mycues.now = 1;",
    ] {
        assert!(!writes_cues(r), "a read: {r}");
    }
}
