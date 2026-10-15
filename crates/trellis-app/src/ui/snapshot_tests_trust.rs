//! The repository trust prompt (settings and hooks that run programs, and the two choices) and
//! the toolbar's Untrusted chip that brings it back.

use super::snapshot_tests::{git, harness, reload, remove_fixture, settings, with_repo};
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::{TrustPrompt, record};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::LoadOptions;
use trellis_core::settings::{RepoTrust, ThemeChoice};
use trellis_core::trust::{ConfigCommand, SubmoduleScan, TrustScan};

fn with_prompt(theme: ThemeChoice, scan: TrustScan) -> AppState {
    prompt_state(theme, scan, false)
}

fn prompt_state(theme: ThemeChoice, scan: TrustScan, refused: bool) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let workdir = PathBuf::from("C:/work/tools");
    st.modal = Some(Modal::Trust(TrustPrompt { name: "tools".into(), real: workdir.clone(), workdir, scan, refused }));
    st
}

fn setting(key: &str, value: &str) -> ConfigCommand {
    ConfigCommand { key: key.into(), value: value.into() }
}

#[test]
fn trust_prompt() {
    let scan = TrustScan {
        config: vec![
            setting("core.sshCommand", "ssh -i ~/.ssh/deploy_key"),
            setting("credential.helper", "!f() { echo password=x; }; f"),
            setting("core.fsmonitor", ".git/fsmon.sh"),
        ],
        hooks: vec!["post-checkout".into(), "pre-commit".into()],
        hooks_dir: Some(PathBuf::from("C:/work/tools/.git/hooks")),
        submodules: Vec::new(),
    };
    let mut h = harness(with_prompt(ThemeChoice::Dark, scan), true);
    h.run();
    h.check_snapshot("trust_prompt");
    assert!(h.query_by_label("core.sshCommand = ssh -i ~/.ssh/deploy_key").is_some());
    assert!(h.query_by_label("in .git/hooks").is_some());
    h.get_by_label("Don't trust").click();
    h.run();
    let decided = h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Decide { trusted: false, .. })));
    assert!(decided, "Don't trust sends a decision");
}

#[test]
fn trust_prompt_hooks_only() {
    let scan = TrustScan {
        config: vec![setting("core.hooksPath", ".githooks"), setting("core.sshCommand", &format!("ssh {}", "x".repeat(196)))],
        hooks: vec!["pre-push".into()],
        hooks_dir: Some(PathBuf::from("C:/work/tools/.githooks")),
        submodules: Vec::new(),
    };
    let mut h = harness(with_prompt(ThemeChoice::Light, scan), false);
    h.run();
    h.check_snapshot("trust_prompt_hooks_only");
    assert!(h.query_by_label_contains("xxx…").is_some(), "the long value is cut");
    assert!(h.query_by_label("in .githooks").is_some());
    h.get_by_label("Trust").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Decide { trusted: true, .. }))));
}

#[test]
fn trust_prompt_submodules() {
    // Nothing in the top repository; one submodule sets an fsmonitor hook, a nested one has a hook.
    let lib = TrustScan { config: vec![setting("core.fsmonitor", ".git/fsmon.sh")], ..TrustScan::default() };
    let inner = TrustScan {
        hooks: vec!["post-merge".into()],
        hooks_dir: Some(PathBuf::from("C:/work/tools/.git/modules/deps/lib/modules/inner/hooks")),
        ..TrustScan::default()
    };
    let scan = TrustScan {
        hooks_dir: Some(PathBuf::from("C:/work/tools/.git/hooks")),
        submodules: vec![
            SubmoduleScan { path: "deps/lib".into(), scan: lib },
            SubmoduleScan { path: "deps/lib/inner".into(), scan: inner },
        ],
        ..TrustScan::default()
    };
    let mut h = harness(with_prompt(ThemeChoice::Dark, scan), true);
    h.run();
    h.check_snapshot("trust_prompt_submodules");
    assert!(h.query_by_label("In submodule deps/lib").is_some());
    assert!(h.query_by_label("core.fsmonitor = .git/fsmon.sh").is_some());
    assert!(h.query_by_label("in .git/modules/deps/lib/modules/inner/hooks").is_some());
}

#[test]
fn trust_prompt_refused() {
    let mut h = harness(prompt_state(ThemeChoice::Light, TrustScan::default(), true), false);
    h.run();
    h.check_snapshot("trust_prompt_refused");
    assert!(h.query_by_label("You chose not to trust it before.").is_some());
    assert!(h.query_by_label("Nothing in its settings or hooks runs programs right now.").is_some());
}

/// Gives the fixture at `dir` a core.sshCommand and loads it again, so its git is held.
fn make_untrusted(st: &mut AppState, dir: &Path) -> PathBuf {
    git(dir, &["config", "core.sshCommand", "ssh -v"], "2026-01-05T10:00:00+00:00");
    reload(st, dir, &LoadOptions::all(500));
    st.snap.as_ref().unwrap().real_workdir.clone()
}

#[test]
fn toolbar_untrusted_tooltip() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "toolbar-untrusted");
    let real = make_untrusted(&mut st, &dir);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Untrusted").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("won't fetch in the background").is_some());
    h.check_snapshot("toolbar_untrusted_tooltip");
    h.get_by_label("Untrusted").click();
    h.run();
    let asked = h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Ask { real: r }) if *r == real));
    assert!(asked, "a click asks again for this repository");
    remove_fixture(&dir);
}

#[test]
fn toolbar_untrusted_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "toolbar-untrusted-light");
    st.settings.toolbar_labels = false;
    let real = make_untrusted(&mut st, &dir);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("toolbar_untrusted_light");
    h.get_by_label("Untrusted").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("You chose not to trust").is_some());
    remove_fixture(&dir);
}

#[test]
fn no_chip_for_a_clean_or_trusted_repository() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "toolbar-trusted");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Untrusted").is_none(), "clean");
    st = std::mem::replace(&mut h.state_mut().0, AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now()));
    drop(h);
    let real = make_untrusted(&mut st, &dir);
    record(&mut st.settings, &real, RepoTrust::Trusted);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Untrusted").is_none(), "trusted");
    remove_fixture(&dir);
}
