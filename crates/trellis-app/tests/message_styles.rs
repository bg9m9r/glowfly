//! Keeps banners and form messages in one style. Only the banner widget (`widgets/banner.rs`)
//! fills with the banner colour; the Git LFS note in a diff is a box inside a pane, not a banner,
//! and keeps it too. Every dialog (`support/dialogs.rs`, the list `controls.rs` checks) and the
//! preference pages with forms leave red and amber to `widgets/form_message.rs`: a problem goes
//! through `form_message::field` or `form`. The dialogs that colour a list row's status (not a
//! form message) are named in `ROW_STATUS`.
//!
//! A dialog's field problem waits until the field is edited or a submit is tried
//! (`form_message::field` with the form's `Touched`), so no dialog calls `form_message::line`,
//! which shows a message at once; only the credential prompt does, for the answer to a try.
//!
//! Test code is skipped, as in `controls.rs`.

#[path = "support/dialogs.rs"]
mod dialogs;
#[path = "support/views.rs"]
mod views;

use dialogs::DIALOG_FILES;
use views::{code_lines, view_sources};

/// The files that may fill with `color::banner()`.
const BANNER_ALLOWED: &[&str] = &["ui/widgets/banner.rs", "ui/diff/lfs_note.rs"];

/// Dialogs whose red or amber marks a row's status in a list, not a form's message.
const ROW_STATUS: &[&str] = &["ui/conflict_check.rs", "ui/release_notes.rs", "ui/repo_finder.rs", "ui/repo_sync.rs"];

/// Forms outside the dialogs whose messages go through `form_message`.
const PAGE_FORMS: &[&str] = &["ui/prefs/gitflow.rs", "ui/prefs/profiles.rs", "ui/prefs/trello.rs"];

/// The dialog that may show a field message at once: a credential prompt's retry note answers
/// the last try.
const LINE_ALLOWED: &[&str] = &["ui/askpass.rs"];

/// The colours a form leaves to `form_message`.
const FORM_DENIED: &[&str] = &["color::red()", "color::amber()"];

/// Whether the file at `rel` must leave red and amber to `form_message`.
fn is_form(rel: &str) -> bool {
    (DIALOG_FILES.contains(&rel) && !ROW_STATUS.contains(&rel)) || PAGE_FORMS.contains(&rel)
}

/// Problems in the file at `rel` (under `src/`), as "line: message".
fn violations(rel: &str, src: &str) -> Vec<String> {
    let banner_ok = BANNER_ALLOWED.contains(&rel);
    let form = is_form(rel);
    let dialog = DIALOG_FILES.contains(&rel) && !LINE_ALLOWED.contains(&rel);
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
        if dialog && line.contains("form_message::line(") {
            found
                .push(format!("{}: `form_message::line(` shows a problem at once; use form_message::field with the form's Touched", i + 1));
        }
    }
    found
}

#[test]
fn banners_and_form_messages_use_the_shared_widgets() {
    let sources = view_sources();
    for f in DIALOG_FILES.iter().chain(BANNER_ALLOWED).chain(ROW_STATUS).chain(PAGE_FORMS).chain(LINE_ALLOWED) {
        assert!(sources.iter().any(|(rel, _)| rel == f), "{f} is still a view file");
    }
    for f in ROW_STATUS.iter().chain(LINE_ALLOWED) {
        assert!(DIALOG_FILES.contains(f), "{f} is still a dialog");
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
    assert!(violations("ui/conflict_check.rs", src).is_empty(), "a row's status");
    assert!(violations("ui/prefs/trello.rs", src).len() == 2, "a page's form");
}

#[test]
fn every_dialog_is_checked_for_coloured_problem_lines() {
    let src = "fn a(ui: &mut Ui) {\n    ui.label(RichText::new(e).color(color::red()));\n}\n";
    let unchecked: Vec<&str> =
        DIALOG_FILES.iter().copied().filter(|f| !ROW_STATUS.contains(f)).filter(|f| violations(f, src).is_empty()).collect();
    assert!(unchecked.is_empty(), "dialogs the colour check skips: {unchecked:?}");
    assert_eq!(violations("ui/trust_dialog.rs", src), ["2: `color::red()`; use form_message::field or form"]);
}

#[test]
fn the_check_catches_a_problem_shown_at_once() {
    let src = "fn a(ui: &mut Ui) {\n    form_message::line(ui, Message::error(p));\n}\n";
    assert_eq!(violations("ui/forge_filter_dialog.rs", src).len(), 1);
    assert_eq!(violations("ui/issue/create_dialog.rs", src).len(), 1);
    assert!(violations("ui/askpass.rs", src).is_empty(), "the answer to a try");
    assert!(violations("ui/prefs/trello.rs", src).is_empty(), "a failed load on a page");
}
