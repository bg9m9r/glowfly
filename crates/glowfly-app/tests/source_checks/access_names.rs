//! Every control in the view snapshots has a name a screen reader can read. The text snapshots
//! (glowfly-ui's `tests/snapshots/*.txt` and the app's `src/runner_snapshots/*.txt`) list the
//! accessibility tree, one node a line: a text field, combo, checkbox, radio button, slider or
//! button line without a quoted name is a control a screen reader announces only by its role,
//! and a name that opens with a symbol (✔ ▾ ★ ⋯) is read out as that symbol's name.
//!
//! The snapshots with such lines today are counted in `access_names_allow.txt`, one
//! `<snapshot>: <count>` line each. The counts only shrink: a snapshot over its count fails, and
//! so does one under it, until its line is lowered (or removed at 0). Name a control with a
//! form row's label (`prefs::form::row`), `widgets::FieldGroup`, or `access::name_field` /
//! `name_button`; never raise a count.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The roles that need a name.
const CONTROLS: &[&str] = &["TextInput", "MultilineTextInput", "ComboBox", "CheckBox", "RadioButton", "Slider", "Button"];

/// The folders holding text snapshots, from `crates/`.
const SNAPSHOT_DIRS: &[&str] = &["glowfly-ui/tests/snapshots", "glowfly-app/src/runner_snapshots"];

/// Today's snapshots with unnamed or symbol-named controls, and how many each has.
const ALLOWED: &str = include_str!("access_names_allow.txt");

/// What is wrong with snapshot line `line`, if it is a control's: no name, or one opening with a
/// symbol.
fn problem(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    let (role, rest) = line.split_once(' ').unwrap_or((line, ""));
    if !CONTROLS.contains(&role) {
        return None;
    }
    let Some(name) = rest.strip_prefix('"') else { return Some("no name") };
    // Letters, digits and plain punctuation ("#12", "[Draft] …") read fine; symbols don't.
    match name.chars().next() {
        Some(c) if !c.is_alphanumeric() && !c.is_ascii() => Some("a name opening with a symbol"),
        _ => None,
    }
}

/// The problems in snapshot `text`, as `<line>: <what>`. The painted text after `painted:` is not
/// the tree and is left out.
fn problems(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|l| *l != "painted:")
        .enumerate()
        .filter_map(|(i, l)| problem(l).map(|p| format!("{}: {p}: {}", i + 1, l.trim())))
        .collect()
}

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates/").to_path_buf()
}

/// Every text snapshot, by its name (`<dir>/<file>` from `crates/`), with its text.
fn snapshots() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for dir in SNAPSHOT_DIRS {
        let Ok(entries) = std::fs::read_dir(crates_dir().join(dir)) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // Not the `.new.txt` and `.diff.txt` a run leaves beside them.
            if name.ends_with(".txt") && !name.ends_with(".new.txt") && !name.ends_with(".diff.txt") {
                let text = std::fs::read_to_string(e.path()).expect("a readable snapshot").replace("\r\n", "\n");
                out.insert(format!("{dir}/{name}"), text);
            }
        }
    }
    out
}

fn allowed() -> BTreeMap<String, usize> {
    ALLOWED
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (file, n) = l.rsplit_once(": ").expect("`<snapshot>: <count>`");
            (file.to_owned(), n.trim().parse().expect("a count"))
        })
        .collect()
}

#[test]
fn every_control_in_the_snapshots_has_a_readable_name() {
    let shots = snapshots();
    assert!(shots.len() > 500, "found the text snapshots");
    let allowed = allowed();
    let mut out = Vec::new();
    for (file, text) in &shots {
        let found = problems(text);
        let ceiling = allowed.get(file).copied().unwrap_or(0);
        if found.len() > ceiling {
            out.push(format!("{file}: {} controls without a readable name (allowed {ceiling}):", found.len()));
            out.extend(found.iter().map(|p| format!("    {p}")));
        } else if found.len() < ceiling {
            out.push(format!("{file}: down to {} from {ceiling}; lower its line in access_names_allow.txt", found.len()));
        }
    }
    for file in allowed.keys().filter(|f| !shots.contains_key(*f)) {
        out.push(format!("{file}: no such snapshot; remove its line from access_names_allow.txt"));
    }
    assert!(out.is_empty(), "screen-reader names:\n{}", out.join("\n"));
}

#[test]
fn the_check_catches_what_it_should() {
    let text = "window 800x600\nTextInput\n  ComboBox = \"Dusk\"\nCheckBox [off]\nButton \"✔  main\"\nButton \"main, checked out\"\n\
                TextInput \"Commit summary\"\nButton \"#12  Crash\"\nCheckBox \"Änderungen\" [on]\nLabel\nUnknown \"x\"\nTabList\npainted:\n  Button\n";
    let found = problems(text);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].starts_with("2: no name") && found[3].starts_with("5: a name opening with a symbol"), "{found:?}");
    assert_eq!(problem("Slider = 3"), Some("no name"));
    assert_eq!(problem("RadioButton \"Árbol\" [off]"), None);
    assert_eq!(problem("Buttonish"), None);
}
