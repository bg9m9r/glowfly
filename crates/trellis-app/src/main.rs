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
use std::path::PathBuf;

fn main() -> eframe::Result {
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
