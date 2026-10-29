//! Keeps English labels, buttons, headers, captions and page names in sentence case: only the
//! first word starts with a capital, apart from proper nouns and acronyms (Git, GitHub, SSH).
//!
//! The check reads `glowfly-core/locales/en.json` (labels, not the prose of tips, help lines,
//! warnings, hints, bodies, intros and toasts) and the first string literal at the label call
//! sites under `src/ui/` (`row(ui, "`, `buttons::secondary("` and so on). A word after the
//! first that starts with a capital and goes on in lowercase is a problem unless it is a proper
//! noun in `PROPER`, follows a sentence or path break (". ", ": ", "? ", "! ", " > ", " · "), or
//! holds a `{placeholder}` or a digit. Mixed-case and all-caps words (GitHub, macOS, SSH) pass.
//! German capitalises its nouns and Spanish keeps its own casing, so only English is checked.

#[path = "support/views.rs"]
mod views;

use std::collections::BTreeMap;
use views::{code_lines, view_sources};

/// Proper nouns and key names that keep their capital anywhere in a label.
const PROPER: &[&str] = &[
    "Git",
    "Flow",
    "LFS",
    "Credential",
    "Manager",
    "Glowfly",
    "Azure",
    "Boards",
    "Bitbucket",
    "Gitea",
    "Gravatar",
    "Jira",
    "Trello",
    "Windows",
    "Linux",
    "Explorer",
    "Finder",
    "Recycle",
    "Bin",
    "Trash",
    "Esc",
    "Enter",
    "Tab",
    "Backspace",
    "Ctrl",
    "Cmd",
    "Shift",
    "Alt",
    "Option",
    // The terminal key Option or Alt acts as.
    "Meta",
    // The trailer git writes, spelled as git spells it.
    "Signed-off-by",
];

/// The parts of a key that mark its value as prose rather than a label.
const PROSE_ROLES: &[&str] = &["tip", "help", "warn", "hint", "body", "intro", "toast"];

/// Key prefixes whose values are names, each with why. Each must still match a key.
const EXEMPT_KEYS: &[(&str, &str)] = &[
    ("prefs.ui.theme.name.", "theme names are proper names (Garden Dark, High Contrast Light)"),
    ("prefs.terminal.shell.", "shell names are product names (Command Prompt, Git Bash)"),
    ("menu.app.", "the macOS app menu keeps the titles macOS gives those items in every app (Hide Others, Show All)"),
];

/// Label call sites under `src/ui/`; the first string literal after each is checked.
const LABEL_CALLS: &[&str] = &[
    "row(ui, \"",
    "section(ui, \"",
    "section_header(ui, \"",
    "buttons::secondary(\"",
    "buttons::primary(\"",
    "buttons::danger(\"",
    "Button::new(\"",
    ".button(\"",
    "small_button(\"",
    "menu_item(msgs, ui, \"",
    "Dialog::new(cx, \"",
    "header(ui, \"",
];

/// A label is checked when it has at most this many words and no sentence break inside.
const MAX_WORDS: usize = 8;

const BREAKS: &[char] = &['.', ':', '?', '!'];

/// The words of `text` after the first that break sentence case.
fn problems(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found = Vec::new();
    for (i, w) in words.iter().enumerate().skip(1) {
        let prev = words[i - 1];
        if prev.ends_with(BREAKS) || prev == ">" || prev == "·" {
            continue;
        }
        if w.contains('{') || w.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        let core = w.trim_matches(|c: char| !c.is_alphanumeric());
        let mut chars = core.chars();
        let (Some(first), Some(second)) = (chars.next(), chars.next()) else { continue };
        if !first.is_uppercase() || !second.is_lowercase() {
            continue;
        }
        // A proper noun's possessive ("Git's") keeps its capital too.
        let noun = core.strip_suffix("'s").or_else(|| core.strip_suffix("\u{2019}s")).unwrap_or(core);
        if core.chars().skip(1).any(char::is_uppercase) || PROPER.contains(&noun) {
            continue;
        }
        found.push(core.to_owned());
    }
    found
}

/// Whether `text` is short enough to be a label (longer text is prose).
fn is_label(text: &str) -> bool {
    text.split_whitespace().count() <= MAX_WORDS && !text.contains(". ")
}

fn is_prose_key(key: &str) -> bool {
    key.split('.').any(|part| part.split('_').any(|w| PROSE_ROLES.contains(&w)))
}

fn en_values() -> BTreeMap<String, String> {
    // Read at run time, as `views::view_sources` does: a binary built in another worktree keeps
    // that worktree's compile-time path.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    let path = std::path::PathBuf::from(manifest).join("../glowfly-core/locales/en.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("en.json is a flat object of strings")
}

/// The first string literal after each label call on `line`.
fn call_labels(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for call in LABEL_CALLS {
        for (i, _) in line.match_indices(call) {
            let rest = &line[i + call.len()..];
            out.push(rest.split('"').next().unwrap_or(rest));
        }
    }
    out
}

#[test]
fn english_labels_are_sentence_case() {
    let values = en_values();
    for (prefix, why) in EXEMPT_KEYS {
        assert!(values.keys().any(|k| k.starts_with(prefix)), "EXEMPT_KEYS entry matches no key: {prefix} ({why})");
    }
    let mut found = Vec::new();
    for (key, value) in &values {
        if is_prose_key(key) || EXEMPT_KEYS.iter().any(|(p, _)| key.starts_with(p)) || !is_label(value) {
            continue;
        }
        let words = problems(value);
        if !words.is_empty() {
            found.push(format!("{key}: \"{value}\" ({})", words.join(", ")));
        }
    }
    assert!(found.is_empty(), "en.json labels not in sentence case:\n{}", found.join("\n"));
}

#[test]
fn view_labels_are_sentence_case() {
    let mut found = Vec::new();
    for (rel, text) in view_sources() {
        for (i, line) in code_lines(&text).iter().enumerate() {
            for label in call_labels(line).into_iter().filter(|l| is_label(l)) {
                let words = problems(label);
                if !words.is_empty() {
                    found.push(format!("{rel}:{}: \"{label}\" ({})", i + 1, words.join(", ")));
                }
            }
        }
    }
    assert!(found.is_empty(), "labels not in sentence case:\n{}", found.join("\n"));
}

#[test]
fn the_rule_knows_proper_nouns_breaks_and_placeholders() {
    assert_eq!(problems("Stage Hunk"), ["Hunk"]);
    assert_eq!(problems("Unstaged Files (2)"), ["Files"]);
    assert_eq!(problems("Use Hunk's lines"), ["Hunk's"]);
    for ok in [
        "Stage hunk",
        "Also sign out of Git Credential Manager",
        "Preferences > Commit signing",
        "Open {name}",
        "Connect to GitHub",
        "Use Git's own ssh",
        "Key for SSH remotes",
        "Moves: off",
        "Done. Next",
        "Local",
    ] {
        assert!(problems(ok).is_empty(), "{ok}: {:?}", problems(ok));
    }
}

#[test]
fn prose_keys_and_long_text_are_left_alone() {
    assert!(is_prose_key("about.copy_info.tip"));
    assert!(is_prose_key("prefs.general.load_more.help"));
    assert!(is_prose_key("commit.toast.pushed"));
    assert!(!is_prose_key("prefs.general.load_more.label"));
    assert!(!is_label("Fetch Pull And Push Every Branch From Every Remote Here Now"));
    assert!(!is_label("Done. Then Next"));
    assert_eq!(call_labels("ui.add(buttons::secondary(\"Stage Hunk\").small())"), ["Stage Hunk"]);
    assert_eq!(call_labels("row(ui, \"Hooks path\", |ui| {})"), ["Hooks path"]);
}
