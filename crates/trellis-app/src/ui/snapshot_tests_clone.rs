//! The Clone dialog with its shallow options open: a depth and a branch ready to clone, and a
//! future start date with an invalid branch name (red lines, Clone disabled). `render` pins
//! today to 2026-02-01.

use super::snapshot_tests::{render, settings};
use crate::state::clone::{CloneForm, LimitKind};
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::settings::ThemeChoice;

fn with_form(theme: ThemeChoice, form: CloneForm) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.modal = Some(Modal::Clone(form));
    st
}

fn form() -> CloneForm {
    // "." is the test's working folder, so the folder check passes.
    CloneForm { url: "https://example.com/team/app.git".into(), shallow: true, ..CloneForm::new(".".into()) }
}

#[test]
fn clone_shallow_depth() {
    let f = CloneForm { limit: LimitKind::Depth, depth: "1".into(), branch: "release/2.0".into(), ..form() };
    render("clone_shallow_depth", with_form(ThemeChoice::Dark, f), true);
}

#[test]
fn clone_shallow_since_future() {
    let f = CloneForm { limit: LimitKind::Since, since: "2026-03-01".into(), branch: "bad name".into(), ..form() };
    render("clone_shallow_since_future", with_form(ThemeChoice::Light, f), false);
}
