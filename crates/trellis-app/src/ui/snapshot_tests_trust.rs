//! The repository trust prompt: settings and hooks that run programs, and the two choices.

use super::snapshot_tests::{harness, settings};
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::TrustPrompt;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;
use trellis_core::trust::{ConfigCommand, TrustScan};

fn with_prompt(theme: ThemeChoice, scan: TrustScan) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let workdir = PathBuf::from("C:/work/tools");
    st.modal = Some(Modal::Trust(TrustPrompt { name: "tools".into(), real: workdir.clone(), workdir, scan }));
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
