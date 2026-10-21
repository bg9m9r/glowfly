//! Trellis: a native Git client.
//!
//! Layers (see docs/ARCHITECTURE.md): `state` (plain data) ← `update` (pure state changes,
//! returns effects) → `runner` (executes effects on worker threads) → `msg` results back into
//! `update`; `ui` draws the state with egui and turns input into messages.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli_update;
mod conflict_check;
mod custom_commands;
mod deep_link;
mod effect;
mod file_op;
mod git_env;
mod gitflow;
#[cfg(test)]
mod glyph_check;
mod journal;
#[cfg(test)]
mod journey;
mod keymap;
mod lfs;
mod logging;
mod logs;
mod msg;
mod onboarding;
mod patch;
mod platform;
mod profile;
mod pull_request;
mod range_diff;
mod remote;
mod runner;
mod smoke_probe;
mod ssh;
mod state;
mod submodule;
mod tag;
mod terminal;
mod themes;
mod tools;
mod ui;
mod update;
mod watcher;
mod worktree;

use eframe::egui;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use trellis_core::askpass::{self, HelperError, Outcome};

fn main() -> eframe::Result {
    // Besides the window, git runs Trellis in three modes that exit without opening one: as the
    // sequence editor of an interactive rebase (`--write-todo`), as gpg.program when it signs
    // (marked by `TRELLIS_GPG_WRAPPER`), and as the askpass program. Scripts have two more:
    // `--version` and `--check-update <folder>` (`cli_update`).
    let argv: Vec<OsString> = std::env::args_os().collect();
    // Checked first: the sequence editor runs under git, so it inherits the other modes'
    // variables.
    if argv.get(1).is_some_and(|a| a == "--write-todo") {
        std::process::exit(write_todo(&argv[2..]));
    }
    // The command-line modes for scripts: before logging, settings and the smoke probe, so they
    // write no file of their own.
    if argv.get(1).is_some_and(|a| a == cli_update::VERSION_FLAG) {
        std::process::exit(cli_update::print_version());
    }
    if argv.get(1).is_some_and(|a| a == cli_update::CHECK_FLAG) {
        std::process::exit(cli_update::check_update(&argv[2..]));
    }
    // git signs with its gpg.program: ask for the key's passphrase through the running app.
    // Before the askpass checks, since git passes the askpass variables on to it too.
    if trellis_core::gpg_prompt::is_wrapper_call(&argv, std::env::var_os(trellis_core::gpg_prompt::ENV_WRAPPER).as_deref()) {
        std::process::exit(trellis_core::gpg_prompt::run_wrapper(&argv[1..]));
    }
    // git or ssh asks for credentials: answer through the running app, without a window.
    // git runs the askpass program with the prompt as its only argument, so the environment
    // variable gitcli sets is what selects this mode; `--askpass <prompt>` is the explicit form.
    if argv.get(1).is_some_and(|a| a == "--askpass") {
        std::process::exit(askpass_helper(argv.get(2)));
    }
    if std::env::var_os(askpass::ENV_ADDR).is_some() {
        std::process::exit(askpass_helper(argv.get(1)));
    }
    // Only the window logs: the helper modes above exit before this.
    let logs = logging::init();
    // Before any repository opens: lets libgit2 open partial clones older git made.
    trellis_core::partial::allow_extension();
    // The real-input smoke tests' state file, only when TRELLIS_SMOKE_PROBE asks for it.
    let probe = smoke_probe::from_env();
    // The OS language, read once: "system" in the Language setting follows it.
    trellis_core::i18n::detect_system_language();
    if let Err(e) = trellis_core::settings::migrate_old_config_dir() {
        tracing::warn!("could not move the settings folder from where an earlier version kept it: {e}");
    }
    // `args_os`: a path needn't be UTF-8 (`env::args` would panic on one).
    let start = argv.get(1).cloned().map(start_arg);
    // A link goes to the window already running, when there is one.
    if let Some(StartArg::Link(text)) = &start {
        match trellis_core::instance::hand_off(text) {
            Ok(()) => return Ok(()),
            Err(e) => tracing::debug!("opening a window for the link: {e}"),
        }
    }
    let chrome = state::window_chrome::Chrome::current();
    // Mica behind the drawn Windows frame needs a transparent window, made only where it shows.
    let mica = chrome == state::window_chrome::Chrome::Windows && platform::native_window::mica_capable();
    let options = eframe::NativeOptions {
        // The frame Trellis draws itself, or the system's (`state::window_chrome`).
        viewport: ui::window_viewport(
            chrome,
            mica,
            egui::ViewportBuilder::default()
                .with_title("Trellis")
                .with_inner_size([1440.0, 880.0])
                .with_min_inner_size([960.0, 540.0])
                .with_icon(crate::platform::app_icon())
                // The Wayland app id: matches trellis.desktop (and its StartupWMClass).
                .with_app_id("trellis"),
        ),
        ..Default::default()
    };
    eframe::run_native("Trellis", options, Box::new(move |cc| Ok(Box::new(ui::TrellisApp::new(cc, start, logs, mica, probe)))))
}

/// What the command line asks the new window to open.
pub enum StartArg {
    /// A repository folder, made absolute.
    Path(PathBuf),
    /// A `trellis://` link (forwarded to a running window first).
    Link(String),
}

/// Reads the command-line argument: a link (`trellis:` in any case), else a repository path. A
/// relative path is relative to the shell it was typed in, so it is made absolute here, before
/// the tabs and recent list keep it.
fn start_arg(arg: OsString) -> StartArg {
    if trellis_core::link::looks_like_link(&arg) {
        return StartArg::Link(arg.to_string_lossy().into_owned());
    }
    let p = start_path(arg);
    StartArg::Path(std::path::absolute(&p).unwrap_or(p))
}

/// The repository path given on the command line. On Windows, `trellis "C:\My Repo\"` arrives
/// as `C:\My Repo"`: the C runtime's rules read `\"` as a quote. A Windows file name can't hold
/// `"`, so a trailing one is always that mistake and is dropped (with the backslash it ate).
fn start_path(arg: OsString) -> PathBuf {
    if cfg!(windows)
        && let Some(s) = arg.to_str()
        && let Some(trimmed) = s.strip_suffix('"')
    {
        let trimmed = trimmed.trim_end_matches('"');
        return PathBuf::from(if trimmed.ends_with(':') { format!("{trimmed}\\") } else { trimmed.to_owned() });
    }
    PathBuf::from(arg)
}

/// `--write-todo <prepared> <git todo>`: replaces git's todo list with the prepared one after
/// checking that both list the same commits. On failure the reason goes to `<prepared>.error`
/// for the app to show, and git is told the editor failed (exit status 1).
fn write_todo(args: &[OsString]) -> i32 {
    let [prepared, git_todo, ..] = args else {
        eprintln!("usage: trellis --write-todo <prepared todo> <git todo>");
        return 2;
    };
    match trellis_core::rebase::install_todo(Path::new(prepared), Path::new(git_todo)) {
        Ok(()) => 0,
        Err(e) => {
            let mut error_file = prepared.clone();
            error_file.push(".error");
            if let Err(w) = std::fs::write(&error_file, e.to_string()) {
                eprintln!("{e} (and could not write {}: {w})", Path::new(&error_file).display());
            }
            1
        }
    }
}

/// Askpass helper mode: asks the app to show `prompt` and prints the answer for git or ssh.
/// Exits 0 with the answer, 1 when the prompt was cancelled, timed out or closed, and 2 when
/// the app can't be reached.
fn askpass_helper(prompt: Option<&OsString>) -> i32 {
    use std::io::Write;
    // An informational notice (e.g. touch a security key): ssh doesn't read an answer.
    if std::env::var(askpass::SSH_HINT_ENV).is_ok_and(|h| h == "none") {
        return 0;
    }
    let Some(prompt) = prompt else {
        eprintln!("usage: trellis --askpass <prompt> (with {} and {} set)", askpass::ENV_ADDR, askpass::ENV_TOKEN);
        return 2;
    };
    match askpass::ask_from_env(&prompt.to_string_lossy()) {
        Ok(Outcome::Answer(s)) => {
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{}", s.expose()).and_then(|()| out.flush()).is_err() {
                return 1;
            }
            0
        }
        Ok(Outcome::Cancelled) => {
            eprintln!("Trellis: the credential prompt was cancelled.");
            1
        }
        Ok(Outcome::TimedOut) => {
            eprintln!("Trellis: the credential prompt timed out.");
            1
        }
        // Only signing requests are answered this way; for anything else it means no answer.
        Ok(Outcome::UseSystem) => {
            eprintln!("Trellis: the credential prompt was not answered.");
            1
        }
        Err(HelperError::Closed) => {
            eprintln!("Trellis closed the credential prompt before it was answered.");
            1
        }
        Err(e) => {
            eprintln!("Trellis: could not reach the app for a credential prompt: {e}");
            2
        }
    }
}

#[cfg(test)]
mod start_path_tests {
    use super::*;

    #[test]
    fn a_quoted_trailing_backslash_is_undone_on_windows() {
        let got = start_path(OsString::from(r#"C:\My Repo""#));
        assert_eq!(got, PathBuf::from(if cfg!(windows) { r"C:\My Repo" } else { r#"C:\My Repo""# }));
        if cfg!(windows) {
            // `"D:\"` names the drive's root, not the current folder on D:.
            assert_eq!(start_path(OsString::from(r#"D:""#)), PathBuf::from(r"D:\"));
        }
        assert_eq!(start_path(OsString::from("/w/repo")), PathBuf::from("/w/repo"));
    }

    #[test]
    fn links_and_paths_are_told_apart() {
        let link = |s: &str| matches!(start_arg(OsString::from(s)), StartArg::Link(t) if t == s);
        assert!(link("trellis://commit/0123456789abcdef0123456789abcdef01234567?root=0123456789abcdef0123456789abcdef01234567"));
        assert!(link("TRELLIS://repo?root=0123456789abcdef0123456789abcdef01234567"));
        // The Windows trailing-quote repair is for paths only.
        assert!(link(r#"trellis://tag/v1?url=x""#));
        let dir = trellis_core::paths::temp_dir().join("trellis-start-arg");
        assert!(matches!(start_arg(dir.clone().into_os_string()), StartArg::Path(p) if p == dir));
        assert!(matches!(start_arg(OsString::from("trellis")), StartArg::Path(p) if p.is_absolute()));
    }

    #[cfg(unix)]
    #[test]
    fn a_path_that_is_not_utf8_is_kept() {
        use std::os::unix::ffi::OsStrExt;
        let arg = std::ffi::OsStr::from_bytes(b"/w/caf\xe9").to_os_string();
        assert_eq!(start_path(arg.clone()), PathBuf::from(arg));
    }
}
