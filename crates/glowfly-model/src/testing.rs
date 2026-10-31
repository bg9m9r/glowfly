//! The test helpers every test in the crate may use, none of which runs the runner: a fresh
//! state and the fixture repositories (`scratch_repo`, `long_repo`). The ones that run effects
//! (`drive`, `finish`) are in `flows/helpers.rs`.

use crate::state::AppState;
use glowfly_core::gitcli::args;
use glowfly_core::settings::Settings;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A fresh state with default settings and no recent repositories.
pub fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

pub use glowfly_core::testkit::{git, set_identity};

/// A repository with one commit and one modified file.
pub fn scratch_repo(name: &str) -> PathBuf {
    glowfly_core::partial::allow_extension();
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    set_identity(&dir);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    dir
}

/// A repository whose `main` has `count` empty commits (made by `git fast-import`), one minute
/// apart, oldest first: commit `i` (0 is the oldest) says "commit {i}", the three oldest also
/// "needle", and the oldest and the newest also "beacon". Longer than the smallest commit limit
/// (500), so its graph is cut short.
pub fn long_repo(name: &str, count: usize) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    set_identity(&dir);
    let mut stream = String::new();
    for i in 0..count {
        let mut msg = format!("commit {i}");
        if i < 3 {
            msg.push_str(" needle");
        }
        if i == 0 || i + 1 == count {
            msg.push_str(" beacon");
        }
        msg.push('\n');
        let time = 1_600_000_000 + 60 * i;
        stream.push_str(&format!("commit refs/heads/main\nmark :{}\ncommitter Test <test@example.com> {time} +0000\n", i + 1));
        stream.push_str(&format!("data {}\n{msg}", msg.len()));
        if i > 0 {
            stream.push_str(&format!("from :{i}\n"));
        }
        stream.push('\n');
    }
    let (ok, out) = glowfly_core::gitcli::run_with_input(&dir, &args(&["fast-import", "--quiet"]), &stream, &[]);
    assert!(ok, "fast-import failed: {out}");
    dir
}

/// Undoes `scratch_repo`'s uncommitted edit of a.txt, for tests of checkouts, pulls and merges
/// the auto-stash prompt would otherwise stop to ask about.
pub fn clean_tree(dir: &Path) {
    std::fs::write(
        dir.join("a.txt"),
        "one
",
    )
    .unwrap();
}

/// The shortcut `text`, written as Windows and Linux show it, as this system shows it: the
/// Command modifier reads "Cmd" and Alt reads "Option" on macOS (`KeyLabels`). Plain Ctrl
/// chords (Ctrl+`) have no business going through this.
pub fn keys(text: &str) -> String {
    if cfg!(target_os = "macos") { text.replacen("Ctrl+", "Cmd+", 1).replace("Alt+", "Option+") } else { text.to_owned() }
}

/// Whether `a` and `b` are the same folder on disk (tests compare a fixture's path, which may be
/// a short 8.3 name on CI, with the one libgit2 reports).
pub fn same_place(a: &Path, b: &Path) -> bool {
    glowfly_core::paths::same_folder(a, b)
}
