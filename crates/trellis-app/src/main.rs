//! Trellis: a native Git client.
//!
//! Layers (see docs/ARCHITECTURE.md): `state` (plain data) ← `update` (pure state changes,
//! returns effects) → `runner` (executes effects on worker threads) → `msg` results back into
//! `update`; `ui` draws the state with egui and turns input into messages.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod effect;
mod msg;
mod platform;
mod runner;
mod state;
mod ui;
mod update;
mod watcher;

use eframe::egui;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn main() -> eframe::Result {
    // Git runs Trellis as the sequence editor of an interactive rebase: install the prepared
    // todo list and exit, without opening a window.
    let argv: Vec<OsString> = std::env::args_os().collect();
    if argv.get(1).is_some_and(|a| a == "--write-todo") {
        std::process::exit(write_todo(&argv[2..]));
    }
    // RUST_LOG=trellis=debug shows background effects as they run.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "trellis=info".into()))
        .init();
    if let Err(e) = trellis_core::settings::migrate_old_config_dir() {
        tracing::warn!("could not move the settings folder from the app's old name (Riffle): {e}");
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
