//! What waits for a repository to be trusted outside the trust prompt: the built-in terminal's
//! held tab (a shield, why, and Trust and run…), and the Git LFS and Sparse Checkout pages that
//! read nothing with git until then (Review trust…).

use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::snapshot_tests_terminal::with_session;
use super::snapshot_tests_trust::make_untrusted;
use crate::msg::{Msg, TrustMsg};
use crate::state::terminal::SessionStatus;
use crate::state::trust::record;
use crate::state::{PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::{RepoTrust, ThemeChoice};
use trellis_core::sparse_ops::SparseStatus;

#[test]
fn terminal_held() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-held", SessionStatus::Held);
    let real = make_untrusted(&mut st, &dir);
    // Maximised, tall enough for the shield; the light one is a default-height panel, without it.
    st.terminal.maximized = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_held");
    assert!(h.query_by_label("The terminal waits until you trust this repository").is_some());
    assert!(h.query_by_label_contains("Shell prompts often run git").is_some());
    h.get_by_label("Trust and run…").click();
    h.run();
    let asked = h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::AskForTerminal { real: r }) if *r == real));
    assert!(asked, "Trust and run… asks on the shells' behalf");
    h.get_by_label("pwsh \u{b7} demo").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("Waiting until you trust this repository").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_held_refused_light() {
    let (mut st, dir) = with_session(ThemeChoice::Light, "terminal-held-refused", SessionStatus::Held);
    let real = make_untrusted(&mut st, &dir);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("terminal_held_refused_light");
    assert!(h.query_by_label_contains("You chose not to trust this repository.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_lfs_held() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "prefs-lfs-held");
    std::fs::write(dir.join(".gitattributes"), "*.psd filter=lfs diff=lfs merge=lfs -text\n").unwrap();
    let date = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", ".gitattributes"], date);
    git(&dir, &["commit", "-q", "-m", "Track Photoshop files in LFS"], date);
    let real = make_untrusted(&mut st, &dir);
    let mut prefs = PrefsState::new(PrefPage::RepoLfs);
    prefs.lfs.generation = 1;
    prefs.lfs.held = true;
    st.prefs = Some(prefs);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_lfs_held");
    assert!(h.query_by_label("Paused until you trust this repository.").is_some());
    assert!(h.query_by_label("*.psd").is_some(), "the patterns come from .gitattributes");
    assert!(h.query_by_label("Initialize LFS").is_none() && h.query_by_label("Track").is_none());
    assert!(h.query_by_label("Untrack").is_none());
    h.get_by_label("Review trust…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Ask { real: r }) if *r == real)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_sparse_held() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "prefs-sparse-held");
    let real = make_untrusted(&mut st, &dir);
    let mut prefs = PrefsState::new(PrefPage::RepoSparse);
    prefs.sparse.generation = 1;
    prefs.sparse.held = true;
    prefs.sparse.status = Some(SparseStatus { enabled: true, held: true, tracked: 340, skipped: 328, ..Default::default() });
    st.prefs = Some(prefs);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("prefs_sparse_held");
    assert!(h.query_by_label_contains("Trellis lists the sparse paths with git").is_some());
    assert!(h.query_by_label("Folders (cone mode)").is_none() && h.query_by_label("Reapply").is_none());
    h.get_by_label("Review trust…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Ask { real: r }) if *r == real)));
    drop(h);
    remove_fixture(&dir);
}
