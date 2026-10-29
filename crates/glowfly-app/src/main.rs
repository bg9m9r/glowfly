//! Glowfly: a native Git client.
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
use glowfly_core::askpass::{self, HelperError, Outcome};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

fn main() -> eframe::Result {
    // Besides the window, git runs Glowfly in three modes that exit without opening one: as the
    // sequence editor of an interactive rebase (`--write-todo`), as gpg.program when it signs
    // (marked by `GLOWFLY_GPG_WRAPPER`), and as the askpass program. Scripts have two more:
    // `--version` and `--check-update <folder>` (`cli_update`), and Install and restart starts a
    // copy of Glowfly as its helper with `--finish-update <plan>`.
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
    // The update helper writes only its own log.
    if argv.get(1).is_some_and(|a| a == cli_update::FINISH_FLAG) {
        std::process::exit(cli_update::finish_update(&argv[2..]));
    }
    // git signs with its gpg.program: ask for the key's passphrase through the running app.
    // Before the askpass checks, since git passes the askpass variables on to it too.
    if glowfly_core::gpg_prompt::is_wrapper_call(&argv, std::env::var_os(glowfly_core::gpg_prompt::ENV_WRAPPER).as_deref()) {
        std::process::exit(glowfly_core::gpg_prompt::run_wrapper(&argv[1..]));
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
    // The system's CJK fonts, searched while the window is made; the first frame waits for them.
    platform::system_fonts::start();
    // Before any repository opens: lets libgit2 open partial clones older git made.
    glowfly_core::partial::allow_extension();
    // The installer checks' state file, only when GLOWFLY_SMOKE_PROBE asks for it.
    let probe = smoke_probe::from_env();
    // The OS language, read once: "system" in the Language setting follows it.
    glowfly_core::i18n::detect_system_language();
    // `args_os`: a path needn't be UTF-8 (`env::args` would panic on one).
    let start = argv.get(1).filter(|a| !is_process_serial(a, cfg!(target_os = "macos"))).cloned().map(start_arg);
    // Whether another window takes links and folders, taking that over when none does.
    let claimed = glowfly_core::instance::claim();
    // A link or a folder goes to the window already running, when there is one.
    if let Some(item) = &start
        && forwarded(item, &claimed)
    {
        return Ok(());
    }
    // Before eframe starts AppKit: the Apple event that started Glowfly.app comes before the
    // window exists.
    platform::open_events::install(start.as_ref());
    // Inside Flatpak, libgit2 reads the host's ~/.config/git like the git it starts does.
    if let Err(e) = glowfly_core::flatpak::init() {
        tracing::warn!("{e}");
    }
    // Before any window exists: an installed Windows copy takes the id its shortcuts carry.
    platform::claim_app_id();
    let chrome = state::window_chrome::Chrome::current();
    // Mica behind the drawn Windows frame needs a transparent window, made only where it shows.
    let mica = chrome == state::window_chrome::Chrome::Windows && platform::native_window::mica_capable();
    let options = eframe::NativeOptions {
        // The frame Glowfly draws itself, or the system's (`state::window_chrome`).
        viewport: ui::window_viewport(
            chrome,
            mica,
            egui::ViewportBuilder::default()
                .with_title("Glowfly")
                .with_inner_size([1440.0, 880.0])
                .with_min_inner_size([960.0, 540.0])
                .with_icon(crate::platform::app_icon())
                // The Wayland app id: matches glowfly.desktop (and its StartupWMClass), or the
                // Flatpak's io.github.bg9m9r.glowfly.desktop.
                .with_app_id(glowfly_core::app_identity::desktop_entry()),
        ),
        // On macOS, no default app menu: Glowfly's own replaces it (`platform::native_menu`).
        event_loop_builder: platform::native_menu::event_loop_hook(),
        ..Default::default()
    };
    let ran = eframe::run_native("Glowfly", options, Box::new(move |cc| Ok(Box::new(ui::GlowflyApp::new(cc, start, logs, mica, probe)))));
    // The window has closed: the copies this session gave the diff tool go now, not at the
    // 24-hour sweep.
    glowfly_core::external_tools::temp::remove_own_sessions();
    ran
}

/// What the command line asks the new window to open; either goes to a window already running
/// first (`forwarded`).
#[derive(Clone, Debug, PartialEq)]
pub enum StartArg {
    /// A repository folder, made absolute.
    Path(PathBuf),
    /// A `glowfly://` link.
    Link(String),
}

impl StartArg {
    /// The message that opens it: the same message for the command line and for what the OS
    /// hands over.
    pub fn into_msg(self) -> msg::Msg {
        match self {
            StartArg::Path(p) => msg::Msg::Repo(msg::RepoMsg::Open(p)),
            StartArg::Link(t) => msg::Msg::DeepLink(deep_link::DeepLinkMsg::Open(t)),
        }
    }

    /// What a second process hands to the running window for it.
    fn handed(&self) -> glowfly_core::instance::Handed {
        match self {
            StartArg::Path(p) => glowfly_core::instance::Handed::Folder(p.clone()),
            StartArg::Link(t) => glowfly_core::instance::Handed::Link(t.clone()),
        }
    }
}

/// Hands `item` to the window that holds the claim on links and folders when another does
/// (`glowfly_core::instance`): whether that window took it. On any failure this process opens
/// its own window, as it would with none running.
fn forwarded(item: &StartArg, claimed: &std::io::Result<glowfly_core::instance::Claimed>) -> bool {
    use glowfly_core::instance::{self, Claimed};
    let wait = match claimed {
        // This is the only window: nothing to hand over to.
        Ok(Claimed::Here) => return false,
        // It may still be starting: give it a moment to listen.
        Ok(Claimed::Elsewhere) => instance::START_WAIT,
        // Nothing is known about other windows: one try at the address on file.
        Err(e) => {
            tracing::debug!("can't tell whether Glowfly is running: {e}");
            std::time::Duration::ZERO
        }
    };
    match instance::hand_off(&item.handed(), wait) {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!("opening a window for it: {e}");
            false
        }
    }
}

/// Finder's process serial number argument (`-psn_0_12345`), which macOS puts on the command
/// line of some launches, such as the first one after a download. It never names a path.
fn is_process_serial(arg: &OsStr, macos: bool) -> bool {
    macos && arg.to_str().is_some_and(|s| s.starts_with("-psn_"))
}

/// Reads the command-line argument: a link (`glowfly:` in any case), else a repository path. A
/// relative path is relative to the shell it was typed in, so it is made absolute here, before
/// the tabs and recent list keep it.
fn start_arg(arg: OsString) -> StartArg {
    if glowfly_core::link::looks_like_link(&arg) {
        return StartArg::Link(arg.to_string_lossy().into_owned());
    }
    let p = start_path(arg);
    StartArg::Path(std::path::absolute(&p).unwrap_or(p))
}

/// The repository path given on the command line. On Windows, `glowfly "C:\My Repo\"` arrives
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
        eprintln!("usage: glowfly --write-todo <prepared todo> <git todo>");
        return 2;
    };
    match glowfly_core::rebase::install_todo(Path::new(prepared), Path::new(git_todo)) {
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
        eprintln!("usage: glowfly --askpass <prompt> (with {} and {} set)", askpass::ENV_ADDR, askpass::ENV_TOKEN);
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
            eprintln!("Glowfly: the credential prompt was cancelled.");
            1
        }
        Ok(Outcome::TimedOut) => {
            eprintln!("Glowfly: the credential prompt timed out.");
            1
        }
        // Only signing requests are answered this way; for anything else it means no answer.
        Ok(Outcome::UseSystem) => {
            eprintln!("Glowfly: the credential prompt was not answered.");
            1
        }
        Err(HelperError::Closed) => {
            eprintln!("Glowfly closed the credential prompt before it was answered.");
            1
        }
        Err(e) => {
            eprintln!("Glowfly: could not reach the app for a credential prompt: {e}");
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
        assert!(link("glowfly://commit/0123456789abcdef0123456789abcdef01234567?root=0123456789abcdef0123456789abcdef01234567"));
        assert!(link("GLOWFLY://repo?root=0123456789abcdef0123456789abcdef01234567"));
        // The Windows trailing-quote repair is for paths only.
        assert!(link(r#"glowfly://tag/v1?url=x""#));
        let dir = glowfly_core::paths::temp_dir().join("glowfly-start-arg");
        assert!(matches!(start_arg(dir.clone().into_os_string()), StartArg::Path(p) if p == dir));
        assert!(matches!(start_arg(OsString::from("glowfly")), StartArg::Path(p) if p.is_absolute()));
    }

    #[test]
    fn start_arguments_become_the_open_messages() {
        use crate::deep_link::DeepLinkMsg;
        use crate::msg::{Msg, RepoMsg};
        let p = glowfly_core::paths::temp_dir().join("glowfly-start-msg");
        assert!(matches!(StartArg::Path(p.clone()).into_msg(), Msg::Repo(RepoMsg::Open(q)) if q == p));
        let t = "glowfly://tag/v1?url=x".to_owned();
        assert!(matches!(StartArg::Link(t.clone()).into_msg(), Msg::DeepLink(DeepLinkMsg::Open(u)) if u == t));
    }

    #[test]
    fn finders_process_serial_argument_is_not_a_path() {
        let psn = OsStr::new("-psn_0_12345");
        assert!(is_process_serial(psn, true));
        assert!(!is_process_serial(psn, false), "only macOS adds it");
        assert!(!is_process_serial(OsStr::new("/w/-psn_x"), true));
        assert!(!is_process_serial(OsStr::new("glowfly://repo?root=x"), true));
    }

    #[cfg(unix)]
    #[test]
    fn a_path_that_is_not_utf8_is_kept() {
        use std::os::unix::ffi::OsStrExt;
        let arg = std::ffi::OsStr::from_bytes(b"/w/caf\xe9").to_os_string();
        assert_eq!(start_path(arg.clone()), PathBuf::from(arg));
    }
}
