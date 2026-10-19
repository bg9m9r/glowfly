//! Keeps banners and form messages in one style. Only the banner widget (`widgets/banner.rs`)
//! fills with the banner colour; the Git LFS note in a diff is a box inside a pane, not a banner,
//! and keeps it too. The forms converted to `widgets/form_message.rs` don't colour their own
//! problem lines red or amber: a problem goes through `form_message::field` or `form`.
//!
//! Test code is skipped, as in `controls.rs`.

#[path = "support/views.rs"]
mod views;

use views::{code_lines, view_sources};

/// The files that may fill with `color::banner()`.
const BANNER_ALLOWED: &[&str] = &["ui/widgets/banner.rs", "ui/diff/lfs_note.rs"];

/// The forms whose messages go through `form_message`.
const FORM_FILES: &[&str] = &["ui/branch_dialogs.rs", "ui/tag_dialog.rs"];

/// The colours a converted form leaves to `form_message`.
const FORM_DENIED: &[&str] = &["color::red()", "color::amber()"];

/// Problems in the file at `rel` (under `src/`), as "line: message".
fn violations(rel: &str, src: &str) -> Vec<String> {
    let banner_ok = BANNER_ALLOWED.contains(&rel);
    let form = FORM_FILES.contains(&rel);
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        if !banner_ok && line.contains("color::banner()") {
            found.push(format!("{}: `color::banner()`; use widgets::banner", i + 1));
        }
        for pat in FORM_DENIED.iter().filter(|_| form) {
            if line.contains(pat) {
                found.push(format!("{}: `{pat}`; use form_message::field or form", i + 1));
            }
        }
    }
    found
}

#[test]
fn banners_and_form_messages_use_the_shared_widgets() {
    let sources = view_sources();
    for f in FORM_FILES.iter().chain(BANNER_ALLOWED) {
        assert!(sources.iter().any(|(rel, _)| rel == f), "{f} is still a view file");
    }
    let mut problems = Vec::new();
    for (rel, text) in &sources {
        problems.extend(violations(rel, text).into_iter().map(|v| format!("{rel}:{v}")));
    }
    assert!(problems.is_empty(), "banner or form message drawn by hand:\n{}", problems.join("\n"));
}

#[test]
fn the_check_catches_a_hand_made_banner() {
    let src = "fn a(ui: &mut Ui) {\n    Frame::new().fill(color::banner());\n}\n";
    assert_eq!(violations("ui/op_strip.rs", src), ["2: `color::banner()`; use widgets::banner"]);
    assert!(violations("ui/widgets/banner.rs", src).is_empty());
    assert!(violations("ui/diff/lfs_note.rs", src).is_empty());
    assert!(violations("ui/op_strip.rs", "// color::banner() in a comment\n").is_empty());
}

#[test]
fn the_check_catches_coloured_problem_lines() {
    let src = "fn a(ui: &mut Ui) {\n    ui.label(RichText::new(p).color(color::red()));\n    ui.label(w.color(color::amber()));\n}\n";
    let found = violations("ui/branch_dialogs.rs", src);
    assert_eq!(found, ["2: `color::red()`; use form_message::field or form", "3: `color::amber()`; use form_message::field or form"]);
    assert!(violations("ui/conflict_check.rs", src).is_empty(), "not a converted form");
}
