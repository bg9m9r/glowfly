//! Comparing a text snapshot with its approved file, `tests/snapshots/<name>.txt`, the way
//! kittest compares an image: the new text is always written beside it as `<name>.new.txt`, a
//! mismatch also writes `<name>.diff.txt` (the changed lines, `-` approved and `+` new) and
//! fails, and `UPDATE_SNAPSHOTS=1` accepts the new text instead. The `.new.txt` and `.diff.txt`
//! files are gitignored.

use std::path::{Path, PathBuf};

/// Where the approved snapshots live: this crate's `tests/snapshots`, also for the app crate's
/// view tests (whose working folder is the app's). The images go there too (`super::check_snapshot`).
pub(super) const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots");

/// Whether `UPDATE_SNAPSHOTS` asks to accept new snapshots (kittest's reading of it).
pub(super) fn updating() -> bool {
    std::env::var("UPDATE_SNAPSHOTS").is_ok_and(|v| !matches!(v.as_str(), "false" | "0" | "no" | "off"))
}

/// Compares `text` with the approved snapshot `name` in `dir`, or accepts it when `update`.
pub(super) fn check(dir: &Path, name: &str, text: &str, update: bool) -> Result<(), String> {
    let path = dir.join(format!("{name}.txt"));
    let new_path = dir.join(format!("{name}.new.txt"));
    let diff_path = dir.join(format!("{name}.diff.txt"));
    let _ = std::fs::remove_file(&diff_path);
    let write = |p: &PathBuf| std::fs::write(p, text).map_err(|e| format!("could not write {}: {e}", p.display()));
    write(&new_path)?;
    let approved = std::fs::read_to_string(&path).ok().map(|s| s.replace("\r\n", "\n"));
    if approved.as_deref() == Some(text) {
        return Ok(());
    }
    if update {
        write(&path)?;
        println!("Updated text snapshot: {}", path.display());
        return Ok(());
    }
    let shown = std::path::absolute(&path).unwrap_or(path);
    let Some(approved) = approved else {
        return Err(format!("Missing text snapshot: {}. Run `UPDATE_SNAPSHOTS=1 cargo test -p glowfly` to write it.", shown.display()));
    };
    let diff = line_diff(&approved, text);
    std::fs::write(&diff_path, &diff).map_err(|e| format!("could not write {}: {e}", diff_path.display()))?;
    let changed = diff.lines().count();
    let first: Vec<&str> = diff.lines().take(8).collect();
    Err(format!(
        "'{name}' text snapshot did not match {}: {changed} lines differ (all in {}):\n{}\nRun `UPDATE_SNAPSHOTS=1 cargo test -p glowfly` to accept the new text.",
        shown.display(),
        diff_path.display(),
        first.join("\n")
    ))
}

/// The lines only in `old` (`- `) and only in `new` (`+ `), in order, from a longest common
/// subsequence of the two.
pub(super) fn line_diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // lcs[i][j]: the common length of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, String::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push_str(&format!("- {}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+ {}\n", b[j]));
            j += 1;
        }
    }
    out
}
