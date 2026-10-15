//! Trellis: a native Git client.
//!
//! Layers (see docs/ARCHITECTURE.md): `state` (plain data) ← `update` (pure state changes,
//! returns effects) → `runner` (executes effects on worker threads) → `msg` results back into
//! `update`; `ui` draws the state with egui and turns input into messages.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod effect;
mod file_op;
mod gitflow;
mod keymap;
mod lfs;
mod msg;
mod platform;
mod remote;
mod runner;
mod ssh;
mod state;
mod submodule;
mod tag;
mod terminal;
mod ui;
mod update;
mod watcher;
mod worktree;

use eframe::egui;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use trellis_core::askpass::{self, HelperError, Outcome};

fn main() -> eframe::Result {
    // Git runs Trellis as the sequence editor of an interactive rebase: install the prepared
    // todo list and exit, without opening a window.
    let argv: Vec<OsString> = std::env::args_os().collect();
    // Checked first: the sequence editor runs under git, so it inherits the askpass variables.
    if argv.get(1).is_some_and(|a| a == "--write-todo") {
        std::process::exit(write_todo(&argv[2..]));
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
    // RUST_LOG=trellis=debug shows background effects as they run.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "trellis=info".into()))
        .init();
    if let Err(e) = trellis_core::settings::migrate_old_config_dir() {
        tracing::warn!("could not move the settings folder from where an earlier version kept it: {e}");
    }
    let start = std::env::args().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trellis")
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([960.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native("Trellis", options, Box::new(move |cc| Ok(Box::new(ui::TrellisApp::new(cc, start)))))
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
