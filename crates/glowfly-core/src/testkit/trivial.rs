//! A merge fixture for taking the trivial conflict picks: one conflicted file whose conflicts
//! all have trivial picks, two with a real conflict and one binary.

use super::{git, set_config, write};
use std::path::Path;

/// `base` with its `at`th line (0-based) replaced by `with`.
fn with_line(base: &str, at: usize, with: &str) -> String {
    base.lines().enumerate().map(|(i, l)| format!("{}\n", if i == at { with } else { l })).collect()
}

/// In the repository `dir` (identity set, on `main`), mid-merge of `other`, with these conflicts:
/// `tidy.txt` and `also.txt` differ only in whitespace on one line (ours re-indented, theirs
/// double-spaced), `mixed.txt` has that and a real change ten lines later, and `logo.bin` is
/// binary. Nothing else is committed or modified.
pub fn mixed_trivial_merge(dir: &Path) {
    set_config(dir, &[("core.autocrlf", "false")]);
    let base: String = (1..=14).map(|i| if i == 2 { "let x = a + b;\n".to_owned() } else { format!("line {i}\n") }).collect();
    let side = |s: &str, ws: &str| {
        let text = with_line(&base, 1, ws);
        let mixed = with_line(&text, 12, &format!("line 13 {s}"));
        let bin = format!("\0\x01{s}\0");
        [("tidy.txt", text.clone()), ("also.txt", text), ("mixed.txt", mixed), ("logo.bin", bin)]
    };
    for (f, _) in side("base", "") {
        write(dir, f, if f == "logo.bin" { "\0\x01base\0" } else { &base });
    }
    git(dir, &["add", "tidy.txt", "also.txt", "mixed.txt", "logo.bin"]);
    git(dir, &["commit", "-q", "-m", "base"]);
    git(dir, &["checkout", "-q", "-b", "other"]);
    for (f, t) in side("theirs", "let x =  a + b;") {
        write(dir, f, &t);
    }
    git(dir, &["commit", "-q", "-m", "theirs", "--", "tidy.txt", "also.txt", "mixed.txt", "logo.bin"]);
    git(dir, &["checkout", "-q", "main"]);
    for (f, t) in side("ours", "    let x = a + b;") {
        write(dir, f, &t);
    }
    git(dir, &["commit", "-q", "-m", "ours", "--", "tidy.txt", "also.txt", "mixed.txt", "logo.bin"]);
    let (ok, out) = crate::gitcli::run(dir, &crate::gitcli::args(&["merge", "other"]));
    assert!(!ok, "merge should conflict: {out}");
}
