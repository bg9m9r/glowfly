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
//! Test code is skipped: files named `tests.rs`, `*_tests.rs` or `snapshot_tests*.rs`, modules
//! declared under `#[cfg(test)]`, and inline `#[cfg(test)] mod` blocks.

use std::path::{Path, PathBuf};

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

/// The files every dialog is drawn from: those `ui/modals.rs` dispatches to, and the
/// confirmation, discard question and askpass windows.
const DIALOG_FILES: &[&str] = &[
    "ui/askpass.rs",
    "ui/branch_dialogs.rs",
    "ui/clone_dialog.rs",
    "ui/confirm_dialog.rs",
    "ui/conflict_check.rs",
    "ui/create_pr_dialog.rs",
    "ui/deep_link_dialog.rs",
    "ui/dialog_guard.rs",
    "ui/file_dialogs.rs",
    "ui/forge_filter_dialog.rs",
    "ui/gitflow_dialog.rs",
    "ui/gitflow_finish_dialog.rs",
    "ui/init_dialog.rs",
    "ui/issue/create_dialog.rs",
    "ui/mainline_dialog.rs",
    "ui/menu_bar/about.rs",
    "ui/patch_apply_dialog.rs",
    "ui/patch_save_dialog.rs",
    "ui/publish.rs",
    "ui/rebase/reword_dialog.rs",
    "ui/release_notes.rs",
    "ui/remotes/dialog.rs",
    "ui/remotes/fork_create.rs",
    "ui/remotes/fork_pick.rs",
    "ui/repo_finder.rs",
    "ui/repo_sync.rs",
    "ui/shortcuts.rs",
    "ui/stash_dialog.rs",
    "ui/stash_push_dialog.rs",
    "ui/submodule_dialogs.rs",
    "ui/tab_dialog.rs",
    "ui/tag_dialog.rs",
    "ui/trash_offer_dialog.rs",
    "ui/trust_dialog.rs",
    "ui/undo_dialog.rs",
    "ui/worktree_dialog.rs",
    "ui/worktree_manage_dialog.rs",
    "ui/worktree_move_dialog.rs",
];

/// What dialogs draw through the frame instead, as (pattern, what to use).
const DIALOG_DENIED: &[(&str, &str)] = &[
    ("ui.heading(", "dialog_frame::Dialog"),
    ("ui.button(", "a Footer button or buttons::secondary"),
    ("Button::new(", "a Footer button or widgets::buttons"),
    ("\"Cancel\"", "Footer::cancel"),
    ("\"Close\"", "Footer::close"),
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
    "ui/welcome.rs",
    "ui/welcome_steps.rs",
    "ui/tour.rs",
    "ui/history.rs",
    "ui/blame/header.rs",
    "ui/diff/header.rs",
    "ui/terminal/mod.rs",
    "ui/op_strip.rs",
    "ui/sign_in_banner.rs",
    "ui/commit_box.rs",
    "ui/gitflow_panel.rs",
    "ui/dashboard.rs",
    "ui/graph/search_bar.rs",
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

/// The source with comments and inline `#[cfg(test)] mod … { … }` blocks blanked out, line for
/// line, so line numbers still match the file.
fn code_lines(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
        if lines[i].trim() == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with('{') {
            out.push(String::new());
            i += 1;
            let mut depth = 0i64;
            while i < lines.len() {
                depth += lines[i].matches('{').count() as i64 - lines[i].matches('}').count() as i64;
                out.push(String::new());
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
            continue;
        }
        out.push(lines[i].split("//").next().unwrap_or("").to_owned());
        i += 1;
    }
    out
}

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

fn is_test_file(rel: &str) -> bool {
    let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".rs");
    stem == "tests" || stem.ends_with("_tests") || stem.starts_with("snapshot_tests")
}

/// The modules `src` declares only for tests (`#[cfg(test)]` followed by `mod name;`).
fn test_only_modules(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|w| w[0] == "#[cfg(test)]")
        .filter_map(|w| w[1].strip_prefix("pub(crate) ").or(Some(w[1]))?.strip_prefix("mod ")?.strip_suffix(';'))
        .map(str::to_owned)
        .collect()
}

/// The path (under `src/`, without `.rs`) of module `name` declared in the file at `rel`.
fn child_module(rel: &str, name: &str) -> String {
    let stem = rel.trim_end_matches(".rs");
    match stem.strip_suffix("/mod") {
        Some(dir) => format!("{dir}/{name}"),
        None => format!("{stem}/{name}"),
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every non-test file under `src/ui/`, as (path under `src/` with `/`, text).
fn view_sources() -> Vec<(String, String)> {
    // Read at run time: a binary built in another worktree (shared target dir) keeps that
    // worktree's compile-time path.
    let src =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into())).join("src");
    let mut files = Vec::new();
    rust_files(&src.join("ui"), &mut files);
    assert!(files.len() > 10, "found the files under src/ui");
    let rel = |f: &Path| f.strip_prefix(&src).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
    let sources: Vec<(String, String)> = files.iter().map(|f| (rel(f), std::fs::read_to_string(f).unwrap())).collect();
    let test_only: Vec<String> =
        sources.iter().flat_map(|(rel, text)| test_only_modules(text).into_iter().map(|m| child_module(rel, &m))).collect();
    sources
        .into_iter()
        .filter(|(rel, _)| {
            let module = rel.trim_end_matches(".rs").trim_end_matches("/mod");
            !is_test_file(rel) && !test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/")))
        })
        .collect()
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
fn test_code_is_skipped() {
    assert!(violations("fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b(ui: &mut Ui) { ui.checkbox(&mut on, \"x\"); }\n}\n").is_empty());
    assert_eq!(violations("fn a() {}\nfn b() { ui.checkbox(&mut on, \"x\"); }\n")[0], "2: `.checkbox(`; use controls::checkbox");
    assert!(is_test_file("ui/theme/controls_tests.rs") && is_test_file("ui/snapshot_tests_clone.rs") && is_test_file("ui/tests.rs"));
    assert_eq!(test_only_modules("#[cfg(test)]\nmod snapshot_check;\n"), ["snapshot_check"]);
    assert_eq!(child_module("ui/theme/mod.rs", "x"), "ui/theme/x");
}
