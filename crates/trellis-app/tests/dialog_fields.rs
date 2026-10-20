//! Keeps every dialog text field in a label-field-help group. A dialog's `TextField` sits inside
//! a `FieldGroup`: written inside a `.show(` call (the group's) or inside a call to a helper of
//! the same file that builds the group, never in a row with a label beside it. A list's filter
//! box (a field whose text is a `filter`) stands alone above its list and is exempt.
//!
//! The check reads the source: it follows the calls each `TextField::` is written inside,
//! skipping strings, character literals and comments. Test code is skipped, as in `controls.rs`.

#[path = "support/dialogs.rs"]
mod dialogs;
#[path = "support/views.rs"]
mod views;

use dialogs::DIALOG_FILES;
use views::{code_lines, view_sources};

/// The word characters of an identifier.
fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The name called by the `(` at byte `at` of `line` (empty for a tuple or a macro).
fn called(line: &str, at: usize) -> String {
    let b = line.as_bytes();
    let start = (0..at).rev().take_while(|&i| is_word(b[i])).last().unwrap_or(at);
    line[start..at].to_owned()
}

/// Whether the `TextField::` at the start of `rest` is a list's filter box.
fn is_filter_box(rest: &str) -> bool {
    rest.strip_prefix("TextField::new(").and_then(|r| r.split(')').next()).is_some_and(|arg| arg.trim_end().ends_with("filter"))
}

/// The functions of `lines` that build a `FieldGroup` (or take one), by name.
fn group_helpers(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in lines {
        let t = line.trim_start();
        let t = t.strip_prefix("pub ").or_else(|| t.strip_prefix("pub(super) ")).or_else(|| t.strip_prefix("pub(crate) ")).unwrap_or(t);
        if let Some(rest) = t.strip_prefix("fn ") {
            current = Some(rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_owned());
        }
        if let Some(name) = &current
            && line.contains("FieldGroup")
            && !out.contains(name)
        {
            out.push(name.clone());
        }
    }
    out
}

/// The text fields in a dialog's source outside a `FieldGroup`, as "line: message".
fn ungrouped_fields(src: &str) -> Vec<String> {
    let code = code_lines(src);
    // The raw lines (strings keep their `//`), with test modules blanked out.
    let lines: Vec<&str> = src.lines().zip(&code).map(|(raw, c)| if c.trim().is_empty() { "" } else { raw }).collect();
    let helpers = group_helpers(&lines);
    let mut calls: Vec<String> = Vec::new();
    let mut in_str = false;
    let mut found = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let b = line.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if in_str {
                match c {
                    b'\\' => i += 1,
                    b'"' => in_str = false,
                    _ => {}
                }
                i += 1;
                continue;
            }
            match c {
                b'"' => in_str = true,
                b'/' if b.get(i + 1) == Some(&b'/') => break,
                // A character literal ('x' or '\x'); a lifetime is a lone quote.
                b'\'' if b.get(i + 1) == Some(&b'\\') => i = (i + 2..b.len()).find(|&j| b[j] == b'\'').unwrap_or(b.len()),
                b'\'' if b.get(i + 2) == Some(&b'\'') => i += 2,
                b'(' => calls.push(called(line, i)),
                b')' => {
                    calls.pop();
                }
                b'T' if line.is_char_boundary(i) && line[i..].starts_with("TextField::") => {
                    let grouped = calls.iter().any(|f| f == "show" || helpers.contains(f));
                    if !grouped && !is_filter_box(&line[i..]) {
                        found.push(format!("{}: a TextField outside a FieldGroup", n + 1));
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    found
}

#[test]
fn dialog_fields_sit_in_field_groups() {
    let report: Vec<String> = view_sources()
        .iter()
        .filter(|(rel, _)| DIALOG_FILES.contains(&rel.as_str()))
        .flat_map(|(rel, text)| ungrouped_fields(text).into_iter().map(move |v| format!("src/{rel}:{v}")))
        .collect();
    assert!(
        report.is_empty(),
        "a dialog's text fields sit in a FieldGroup (label above, help or message under), not in a row with a label beside:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_ungrouped_fields() {
    let n = |s: &str| ungrouped_fields(s).len();
    // A label beside the field, as a row.
    assert_eq!(
        n("fn row(ui: &mut Ui) {\n    ui.horizontal(|ui| {\n        ui.label(label);\n        ui.add(TextField::new(text));\n    });\n}"),
        1
    );
    // Bound first and added later, outside the group.
    assert_eq!(n("let field = TextField::new(&mut url);\nlet r = group.show(ui, |ui| ui.add(field));"), 1);
    // A row helper that takes no group.
    assert_eq!(n("fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {}\nrow(ui, \"Labels\", |ui| ui.add(TextField::new(&mut l)));"), 1);
    // Inside the group's show, directly or in a row of its own.
    assert_eq!(n("FieldGroup::new(\"Name\").show(ui, |ui| ui.add(TextField::new(&mut s)));"), 0);
    assert_eq!(
        n("let r = group.show(ui, |ui| {\n    ui.horizontal(|ui| ui.add(TextField::new(&mut s).hint(\"a (b\")))\n        .inner\n});"),
        0
    );
    // Inside a helper of the file that builds the group.
    assert_eq!(
        n("fn cond(ui: &mut Ui) {\n    FieldGroup::new(label).show(ui, add);\n}\ncond(ui, |ui| ui.add(TextField::new(&mut b)));"),
        0
    );
    // A list's filter box stands alone.
    assert_eq!(n("let r = ui.add(TextField::new(&mut p.filter).hint(\"Filter\"));"), 0);
    // Strings, comments and character literals don't count as calls.
    assert_eq!(n("let s = \"show(\";\nlet c = '(';\n// show(\nui.add(TextField::new(&mut s));"), 1);
}
