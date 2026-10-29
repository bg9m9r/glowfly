//! The status bar's live-updates dot: a sheet of its five states (a pixel golden), the status
//! bar with a silent watcher (Slow updates refreshes on a click), and no dot while nothing is
//! watched.

use super::status_item;
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::watch::Probe;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::settings;
use crate::ui::theme::{self, color};
use eframe::egui::{self, Align, Layout, RichText, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn repo() -> PathBuf {
    PathBuf::from("C:/work/glowfly")
}

/// A state watching `repo()`, its probe having found `probe` (None: still out; `failed`: the
/// watcher failed instead).
fn watching(probe: Option<Probe>, failed: bool) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.watch.target = Some(repo());
    if let Some(p) = probe {
        st.watch.probes.insert(repo(), p);
    }
    if failed {
        st.watch.failed.insert(repo());
    }
    st
}

/// A harness of `size` drawing `draw` in the Garden dark palette, as the views' snapshots do.
fn harness(st: AppState, size: egui::Vec2, draw: impl FnMut(&egui::Context, &mut AppState, &mut Vec<Msg>) + 'static) -> H {
    let p = theme::GARDEN_DARK;
    let mut draw = draw;
    crate::ui::snapshot_check::builder().with_size(size).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_palette(&p);
            theme::apply_style(ctx, &p);
            ctx.set_theme(theme::egui_theme(&p));
            draw(ctx, st, msgs);
        },
        (st, Vec::new()),
    )
}

#[test]
fn live_updates_states() {
    let states: [(Option<Probe>, bool); 5] =
        [(None, false), (Some(Probe::Live), false), (Some(Probe::Unknown), false), (Some(Probe::Silent), false), (None, true)];
    let mut h = harness(watching(None, false), vec2(320.0, 180.0), move |ctx, st, msgs| {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(color::toolbar())).show(ctx, |ui| {
            for (probe, failed) in states {
                st.watch.probes.clear();
                st.watch.failed.clear();
                if let Some(p) = probe {
                    st.watch.probes.insert(repo(), p);
                }
                if failed {
                    st.watch.failed.insert(repo());
                }
                let _ = ui.allocate_ui_with_layout(vec2(300.0, 26.0), Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("Glowfly 1.0.0").color(color::faint()));
                    status_item(ui, st, msgs);
                });
                ui.add_space(4.0);
            }
        });
    });
    h.run();
    h.check_snapshot("live_updates_states");
    for name in [
        "Checking that changes on disk show up live.",
        "Live: changes to files show up as they happen.",
        "Glowfly can't write to this repository, so it can't check that changes show up live.",
        "Slow updates",
        "Not live",
    ] {
        assert!(h.query_by_label(name).is_some(), "{name}");
    }
}

fn bar(st: AppState) -> H {
    harness(st, vec2(720.0, 26.0), |ctx, st, msgs| crate::ui::status::status_bar(ctx, st, msgs))
}

#[test]
fn status_bar_live_updates_silent() {
    let mut h = bar(watching(Some(Probe::Silent), false));
    h.run();
    h.check_snapshot("status_bar_live_updates_silent");
    h.get_by_label("Slow updates").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::Refresh))), "a click refreshes now");
}

#[test]
fn a_live_dot_takes_no_click_and_nothing_watched_shows_none() {
    let mut h = bar(watching(Some(Probe::Live), false));
    h.run();
    h.get_by_label("Live: changes to files show up as they happen.").click();
    h.run();
    assert!(h.state().1.is_empty(), "the quiet states are hover-only");
    let mut st = watching(Some(Probe::Live), false);
    st.watch.target = None;
    let mut h = bar(st);
    h.run();
    assert!(h.query_by_label_contains("show up").is_none(), "no dot while nothing is watched");
}
