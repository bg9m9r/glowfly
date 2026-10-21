//! Keeps form controls readable in every theme. egui's own text fields, checkboxes, radio
//! buttons and modals take their colours from egui's widget style, which the palette can't make
//! readable everywhere (a field's edge, an unchecked box and a modal's edge would vanish on some
//! themes). Views under `src/ui/` use the themed helpers instead: `theme::controls::field` /
//! `Field`, `checkbox`, `radio`, `radio_value`, `Check` and `Radio`, and `theme::modal`.
//!
//! A text field built in a `let` and added later is caught by a count: each file needs at least
//! as many `field(` / `Field(` wrappers as lines that make a `TextEdit`.
//!
//! Buttons keep one look too. Every dialog (the files `ui/modals.rs` dispatches to, plus the
//! confirmation, discard question and askpass windows) draws its title, buttons and Cancel or
//! Close through `dialog_frame`: no `ui.heading(`, no egui buttons and no "Cancel" / "Close"
//! literals. Preference pages and panels use `widgets::buttons` rather than hand-made fills
//! (`primary_off` / `danger_off`), egui buttons faded by `add_enabled`, or English "Cancel" /
//! "Close" labels. Nowhere may `add_enabled` wrap a styled button, which would fade it; its own
//! `.enabled` keeps the disabled look. `widgets/buttons.rs` and `common.rs` (the menu items) are
//! exempt.
//!
//! A dialog's text fields are `widgets::TextField` (36 px, tinted, inside a `FieldGroup`): no
//! `controls::field(` or `TextEdit::` in a dialog file. The dialogs for less familiar operations
//! (`GUIDED`) explain them in a side pane: each of their `Dialog::new(` has a
//! `.guide(dialog_guides::..)`, and no other dialog has a `.guide(`, so everyday dialogs keep one
//! plain pane.
//!
//! Every checkbox says what it does beside the box: no `checkbox(.., "")` in any view, and no
//! `Check::without_text` on a preference page (a list row's bare box is labelled by its row).
//!
//! Test code is skipped: files named `tests.rs`, `*_tests.rs` or `snapshot_tests*.rs`, modules
//! declared under `#[cfg(test)]`, and inline `#[cfg(test)] mod` blocks.

#[path = "support/dialogs.rs"]
mod dialogs;
#[path = "support/views.rs"]
mod views;

use dialogs::DIALOG_FILES;
use views::{child_module, code_lines, is_test_file, test_only_modules, view_sources};

/// The files that define the helpers, and may use egui's widgets.
const ALLOWED_FILES: &[&str] = &["ui/theme/controls.rs", "ui/theme/mod.rs"];

/// egui's own controls, as (pattern, the helper to use instead).
const DENIED: &[(&str, &str)] = &[
    (".checkbox(", "controls::checkbox"),
    (".radio(", "controls::radio"),
    (".radio_value(", "controls::radio_value"),
    ("Checkbox::", "controls::Check"),
    ("RadioButton::", "controls::Radio"),
    ("text_edit_singleline", "controls::field"),
    ("text_edit_multiline", "controls::field"),
    ("add(egui::TextEdit", "controls::field"),
    ("add(TextEdit", "controls::field"),
    ("Modal::new(", "theme::modal"),
];

/// What dialogs draw through the frame instead, as (pattern, what to use).
const DIALOG_DENIED: &[(&str, &str)] = &[
    ("ui.heading(", "dialog_frame::Dialog"),
    ("ui.button(", "a Footer button or buttons::secondary"),
    ("Button::new(", "a Footer button or widgets::buttons"),
    ("\"Cancel\"", "Footer::cancel"),
    ("\"Close\"", "Footer::close"),
];

/// What dialogs draw their text fields with instead, as (pattern, what to use).
const DIALOG_FIELD_DENIED: &[(&str, &str)] = &[("controls::field(", "widgets::TextField"), ("TextEdit::", "widgets::TextField")];

/// The dialogs for less familiar operations, which explain them in a side pane.
const GUIDED: &[&str] = &[
    "ui/gitflow_dialog.rs",
    "ui/gitflow_finish_dialog.rs",
    "ui/mainline_dialog.rs",
    "ui/patch_apply_dialog.rs",
    "ui/patch_save_dialog.rs",
    "ui/submodule_dialogs.rs",
    "ui/worktree_dialog.rs",
    "ui/worktree_manage_dialog.rs",
    "ui/worktree_move_dialog.rs",
];

/// Preference pages and panels, as files or folders (ending in `/`). The issue picker's
/// frameless list items are not buttons.
const BUTTON_FILES: &[&str] = &[
    "ui/prefs/",
    "ui/pull_request/",
    "ui/issue/mod.rs",
    "ui/issue/conversation.rs",
    "ui/issue/info.rs",
    "ui/details_banner.rs",
    "ui/conflict_header.rs",
    "ui/rebase/banner.rs",
    "ui/rebase/mod.rs",
    "ui/rebase/editor.rs",
    "ui/tour.rs",
    "ui/history.rs",
    "ui/blame/header.rs",
    "ui/diff/header/",
    "ui/terminal/mod.rs",
    "ui/op_strip.rs",
    "ui/sign_in_banner.rs",
    "ui/commit_box.rs",
    "ui/gitflow_panel.rs",
    "ui/dashboard.rs",
    "ui/graph/search_bar.rs",
    "ui/git_banner.rs",
    "ui/lfs_banner.rs",
    "ui/pane_status.rs",
    "ui/empty_state.rs",
    "ui/graph_empty.rs",
];

/// What pages and panels take from `widgets::buttons` or the catalog instead.
const BUTTON_DENIED: &[(&str, &str)] = &[
    ("color::primary_off()", "buttons::primary(..).enabled(..)"),
    ("color::danger_off()", "buttons::danger(..).enabled(..)"),
    ("\"Cancel\"", "t.tr(\"common.cancel.button\")"),
    ("\"Close\"", "t.tr(\"common.close.button\")"),
];

/// The files that define the button styles and the menu items, which may build egui buttons.
const BUTTON_ALLOWED: &[&str] = &["ui/widgets/buttons.rs", "ui/common.rs"];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// How often `pat` occurs in `line`; a pattern starting with a word character must start a word.
fn count(line: &str, pat: &str) -> usize {
    let word = pat.chars().next().is_some_and(is_word);
    line.match_indices(pat).filter(|&(i, _)| !word || !line[..i].chars().next_back().is_some_and(is_word)).count()
}

/// Problems in one file's source, as "line: message".
fn violations(src: &str) -> Vec<String> {
    let code = code_lines(src);
    let mut found = Vec::new();
    let (mut edits, mut wrappers, mut first_edit) = (0, 0, None);
    for (i, line) in code.iter().enumerate() {
        for (pat, helper) in DENIED {
            if count(line, pat) > 0 {
                found.push(format!("{}: `{pat}`; use {helper}", i + 1));
            }
        }
        if line.contains("TextEdit::singleline") || line.contains("TextEdit::multiline") {
            edits += 1;
            first_edit.get_or_insert(i + 1);
        }
        wrappers += count(line, "field(") + count(line, "Field(");
    }
    if edits > wrappers {
        let at = first_edit.unwrap_or(1);
        found.push(format!("{at}: {edits} lines make a TextEdit but only {wrappers} add one through controls::field / Field"));
    }
    found
}

/// Button problems in the file at `rel` (under `src/`), as "line: message".
fn button_violations(rel: &str, src: &str) -> Vec<String> {
    if BUTTON_ALLOWED.contains(&rel) {
        return Vec::new();
    }
    let dialog = DIALOG_FILES.contains(&rel);
    let panel = BUTTON_FILES.iter().any(|f| if f.ends_with('/') { rel.starts_with(f) } else { rel == *f });
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        let denied = match (dialog, panel) {
            (true, _) => DIALOG_DENIED,
            (false, true) => BUTTON_DENIED,
            _ => &[],
        };
        for (pat, instead) in denied {
            if count(line, pat) > 0 {
                found.push(format!("{}: `{pat}`; use {instead}", i + 1));
            }
        }
        if line.contains("add_enabled(") {
            if line.contains("buttons::") {
                found.push(format!("{}: `add_enabled(` fades a styled button; use its `.enabled(..)`", i + 1));
            } else if panel && line.contains("Button::") {
                found.push(format!("{}: `add_enabled(` with an egui button; use buttons::secondary(..).enabled(..)", i + 1));
            }
        }
    }
    found
}

/// A dialog's text fields not drawn with `TextField`, as "line: message".
fn field_violations(src: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        for (pat, instead) in DIALOG_FIELD_DENIED {
            if count(line, pat) > 0 {
                found.push(format!("{}: `{pat}`; use {instead}", i + 1));
            }
        }
    }
    found
}

/// A side pane where the dialog file at `rel` should or shouldn't have one, as a message.
fn guide_violation(rel: &str, src: &str) -> Option<String> {
    let code = code_lines(src);
    let n = |pat: &str| code.iter().map(|l| count(l, pat)).sum::<usize>();
    if GUIDED.contains(&rel) {
        let (dialogs, guides) = (n("Dialog::new("), n(".guide(dialog_guides::"));
        (dialogs != guides)
            .then(|| format!("{dialogs} dialogs but {guides} `.guide(dialog_guides::..)`; each explains its step in a side pane"))
    } else {
        let guides = n(".guide(");
        (guides > 0).then(|| format!("{guides} `.guide(`; an everyday dialog has no side pane (or add the file to GUIDED)"))
    }
}

/// A checkbox without inline text, as "line: message".
fn bare_checkboxes(rel: &str, src: &str) -> Vec<String> {
    let prefs = rel.starts_with("ui/prefs/");
    let bare = |line: &str| {
        (prefs && line.contains("Check::without_text("))
            || line.match_indices("checkbox(").any(|(i, _)| line[i..].split(')').next().is_some_and(|args| args.ends_with(", \"\"")))
    };
    code_lines(src).iter().enumerate().filter(|(_, l)| bare(l)).map(|(i, _)| format!("{}: a checkbox without inline text", i + 1)).collect()
}

#[test]
fn checkboxes_have_inline_text() {
    let report: Vec<String> = view_sources()
        .iter()
        .flat_map(|(rel, text)| bare_checkboxes(rel, text).into_iter().map(move |v| format!("src/{rel}:{v}")))
        .collect();
    assert!(
        report.is_empty(),
        "preference checkboxes say what they do beside the box (form::check_row):
{}",
        report.join(
            "
"
        )
    );
}

#[test]
fn the_check_catches_bare_checkboxes() {
    let n = |rel: &str, s: &str| bare_checkboxes(rel, s).len();
    assert_eq!(n("ui/prefs/signing.rs", "if controls::checkbox(ui, &mut git.gpgsign, \"\").changed() {"), 1);
    assert_eq!(n("ui/prefs/signing.rs", "ui.add(controls::Check::without_text(&mut on));"), 1);
    assert_eq!(n("ui/prefs/signing.rs", "controls::checkbox(ui, &mut on, \"Sign new commits by default\");"), 0);
    assert_eq!(n("ui/patch_save_dialog.rs", "ui.add(controls::Check::without_text(&mut on));"), 0, "a list row's box");
    assert_eq!(n("ui/stash_push_dialog.rs", "controls::checkbox(ui, &mut on, \"\");"), 1, "anywhere");
}

#[test]
fn views_use_the_themed_controls() {
    let mut report = Vec::new();
    for (rel, text) in &view_sources() {
        if ALLOWED_FILES.contains(&rel.as_str()) {
            continue;
        }
        report.extend(violations(text).into_iter().map(|v| format!("src/{rel}:{v}")));
    }
    assert!(
        report.is_empty(),
        "views draw text fields, checkboxes, radio buttons and modals with the themed helpers in \
         src/ui/theme/controls.rs and theme::modal, so they stay readable in every theme:\n{}",
        report.join("\n")
    );
}

#[test]
fn dialogs_and_panels_use_the_button_styles() {
    let sources = view_sources();
    let missing: Vec<&str> =
        DIALOG_FILES.iter().chain(BUTTON_FILES).filter(|f| !sources.iter().any(|(rel, _)| rel.starts_with(**f))).copied().collect();
    assert!(missing.is_empty(), "listed files that no longer exist (update the lists): {missing:?}");
    let report: Vec<String> =
        sources.iter().flat_map(|(rel, text)| button_violations(rel, text).into_iter().map(move |v| format!("src/{rel}:{v}"))).collect();
    assert!(
        report.is_empty(),
        "dialogs draw their title and buttons through dialog_frame, and pages and panels use \
         widgets::buttons with translated Cancel and Close labels:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_unstyled_buttons() {
    let n = |rel: &str, s: &str| button_violations(rel, s).len();
    let dialog = "ui/trust_dialog.rs";
    assert_eq!(n(dialog, "ui.heading(\"Trust this folder?\");"), 1);
    assert_eq!(n(dialog, "if ui.add(buttons::secondary(\"Cancel\")).clicked() {"), 1);
    assert_eq!(n(dialog, "if ui.button(\"Close\").clicked() {"), 2);
    assert_eq!(n(dialog, "let d = Dialog::new(cx, \"Trust this folder?\").begin(ui);"), 0);
    let panel = "ui/prefs/general.rs";
    assert_eq!(n(panel, "let fill = if on { color::danger() } else { color::danger_off() };"), 1);
    assert_eq!(n(panel, "ui.add_enabled(on, egui::Button::new(\"Reset\"));"), 1);
    assert_eq!(n(panel, "ui.add(buttons::secondary(t.tr(\"common.cancel.button\")).enabled(on));"), 0);
    // Anywhere: add_enabled would fade the style.
    assert_eq!(n("ui/stash.rs", "ui.add_enabled(on, buttons::primary(\"Stash\"));"), 1);
    assert_eq!(n("ui/stash.rs", "ui.add_enabled(on, egui::Button::new(\"Stash\"));"), 0, "not a panel");
    assert_eq!(n("ui/common.rs", "ui.add_enabled(on, buttons::primary(\"x\"));"), 0, "allowed");
}

#[test]
fn the_check_catches_egui_controls() {
    let n = |s: &str| violations(s).len();
    assert_eq!(n("if ui.checkbox(&mut on, \"Sign\").changed() {"), 1);
    assert_eq!(n("ui.radio_value(&mut v, 1, \"One\");"), 1);
    assert_eq!(n("ui.radio(on, \"One\");"), 1);
    assert_eq!(n("ui.add(egui::Checkbox::without_text(&mut on));"), 1);
    assert_eq!(n("ui.add(egui::RadioButton::new(on, \"x\"));"), 1);
    assert_eq!(n("ui.add(egui::TextEdit::singleline(&mut s));"), 2, "and no wrapper");
    assert_eq!(n("ui.text_edit_singleline(&mut s);"), 1);
    assert_eq!(n("egui::Modal::new(id).show(ctx, |ui| {});"), 1);
    // A let-bound edit added without the wrapper.
    assert_eq!(n("let e = egui::TextEdit::multiline(&mut s);\nui.add(e);"), 1);
    // The themed helpers are fine.
    assert_eq!(n("controls::checkbox(ui, &mut on, \"Sign\");"), 0);
    assert_eq!(n("controls::radio_value(ui, &mut v, 1, \"One\");"), 0);
    assert_eq!(n("ui.add(controls::Check::without_text(&mut on));"), 0);
    assert_eq!(n("let e = egui::TextEdit::multiline(&mut s);\ncontrols::field(ui, e);"), 0);
    assert_eq!(n("ui.add_enabled(on, controls::Field(e));\nlet e = egui::TextEdit::multiline(&mut s);"), 0);
    assert_eq!(n("super::theme::modal(ctx, id).show(ctx, |ui| {});"), 0);
    // A local helper named like the wrapper doesn't count as one.
    assert_eq!(n("let e = egui::TextEdit::singleline(&mut s);\nname_field(&ui.add(e), \"Name\");"), 1);
}

#[test]
fn dialogs_use_text_field() {
    let sources = view_sources();
    let report: Vec<String> = sources
        .iter()
        .filter(|(rel, _)| DIALOG_FILES.contains(&rel.as_str()))
        .flat_map(|(rel, text)| field_violations(text).into_iter().map(move |v| format!("src/{rel}:{v}")))
        .collect();
    assert!(
        report.is_empty(),
        "dialog text fields are widgets::TextField inside a FieldGroup (36 px, tinted, label above):\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_plain_dialog_fields() {
    let n = |s: &str| field_violations(s).len();
    assert_eq!(n("let r = controls::field(ui, egui::TextEdit::singleline(&mut s));"), 2);
    assert_eq!(n("let e = egui::TextEdit::multiline(&mut s);"), 1);
    assert_eq!(n("let r = ui.add(TextField::new(&mut s).hint(\"v1.0.0\"));"), 0);
    assert_eq!(n("FieldGroup::new(\"Name\").show(ui, |ui| ui.add(TextField::multiline(&mut s, 3)));"), 0);
}

#[test]
fn guided_dialogs_have_a_side_pane() {
    let sources = view_sources();
    let missing: Vec<&str> = GUIDED.iter().filter(|f| !sources.iter().any(|(rel, _)| rel == *f)).copied().collect();
    assert!(missing.is_empty(), "listed files that no longer exist (update GUIDED): {missing:?}");
    let report: Vec<String> = sources
        .iter()
        .filter(|(rel, _)| DIALOG_FILES.contains(&rel.as_str()) || GUIDED.contains(&rel.as_str()))
        .filter_map(|(rel, text)| guide_violation(rel, text).map(|v| format!("src/{rel}: {v}")))
        .collect();
    assert!(
        report.is_empty(),
        "dialogs for less familiar operations explain them in a side pane, and everyday dialogs have none:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_misplaced_guides() {
    let v = |rel: &str, s: &str| guide_violation(rel, s).is_some();
    let guided = "ui/worktree_move_dialog.rs";
    assert!(!v(guided, "let d = Dialog::new(cx, \"Move\").guide(dialog_guides::worktree_move(cx.t)).begin(ui);"));
    assert!(v(guided, "let d = Dialog::new(cx, \"Move\").begin(ui);"), "a guided dialog without its pane");
    assert!(v(guided, "Dialog::new(cx, \"A\").guide(dialog_guides::a(t));\nDialog::new(cx, \"B\");"), "one of two");
    let everyday = "ui/branch_dialogs.rs";
    assert!(!v(everyday, "let d = Dialog::new(cx, \"New branch\").begin(ui);"));
    assert!(v(everyday, "let d = Dialog::new(cx, \"New branch\").guide(g).begin(ui);"), "an everyday dialog with a pane");
}

#[test]
fn test_code_is_skipped() {
    assert!(violations("fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b(ui: &mut Ui) { ui.checkbox(&mut on, \"x\"); }\n}\n").is_empty());
    assert_eq!(violations("fn a() {}\nfn b() { ui.checkbox(&mut on, \"x\"); }\n")[0], "2: `.checkbox(`; use controls::checkbox");
    assert!(is_test_file("ui/theme/controls_tests.rs") && is_test_file("ui/snapshot_tests_clone.rs") && is_test_file("ui/tests.rs"));
    assert_eq!(test_only_modules("#[cfg(test)]\nmod snapshot_check;\n"), ["snapshot_check"]);
    assert_eq!(child_module("ui/theme/mod.rs", "x"), "ui/theme/x");
}
