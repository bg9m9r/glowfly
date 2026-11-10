//! The trust prompt for a repository whose own config changes how Git connects (TLS checks
//! and proxies), listed apart from the settings that run commands, and git-lfs's programs
//! among those; and for one with a rebase in progress whose steps left run commands (a long one cut,
//! and shown whole), or whose files couldn't be read.

use super::snapshot_tests::harness;
use super::snapshot_tests_trust::{setting, with_prompt};
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::trust::TrustScan;
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::Modal;

#[test]
fn trust_prompt_network() {
    let scan = TrustScan {
        config: vec![
            setting("http.sslVerify", "false"),
            setting("lfs.customtransfer.agent.path", "./bin/agent"),
            setting("remote.origin.proxy", "proxy.example:8080"),
            setting("http.https://example.com/.sslCAInfo", "certs/ca.pem"),
        ],
        ..TrustScan::default()
    };
    let mut h = harness(with_prompt(ThemeChoice::Dark, scan), true);
    h.run();
    h.check_snapshot("trust_prompt_network");
    assert!(h.query_by_label("Settings that run commands").is_some());
    assert!(h.query_by_label("lfs.customtransfer.agent.path = ./bin/agent").is_some());
    assert!(h.query_by_label("Settings that change how Git connects").is_some());
    assert!(h.query_by_label("http.sslVerify = false").is_some());
    assert!(h.query_by_label_contains("turn off certificate checks").is_some());
}

#[test]
fn trust_prompt_steps() {
    let long = format!("exec make {}", "x".repeat(450));
    let scan = TrustScan { steps: vec!["exec make test".into(), "--strategy=mine".into(), long], ..TrustScan::default() };
    let mut h = harness(with_prompt(ThemeChoice::Dark, scan), true);
    h.run();
    h.check_snapshot("trust_prompt_steps");
    assert!(h.query_by_label("Commands the operation in progress runs").is_some());
    assert!(h.query_by_label("exec make test").is_some());
    assert!(h.query_by_label("--strategy=mine").is_some());
    assert!(h.query_by_label_contains("xxx…").is_some(), "the long step is cut");
    assert!(h.query_by_label("60 more characters aren't shown.").is_some(), "and says so");
    h.get_by_label("Show all").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::ToggleStep(2)))));
    assert!(h.query_by_label_contains("Continuing or skipping").is_some());
    assert!(h.query_by_label("Settings that run commands").is_none());
}

#[test]
fn trust_prompt_step_shown_whole() {
    // A step much longer than the dialog is tall, opened with Show all: all of it is there.
    let long = format!("exec make {}end", "word ".repeat(700));
    let scan = TrustScan { steps: vec![long.clone()], ..TrustScan::default() };
    let mut st = with_prompt(ThemeChoice::Dark, scan);
    if let Some(Modal::Trust(p)) = &mut st.modal {
        p.open_step = Some(0);
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("trust_prompt_step_shown_whole");
    assert!(h.query_by_label(&long).is_some(), "the whole step is in the dialog");
    assert!(h.query_by_label_contains("more characters").is_none());
    h.get_by_label("Show less").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::ToggleStep(0)))));
}

#[test]
fn trust_prompt_state_unread() {
    // A rebase whose todo couldn't be read: the job waits each time, saying why.
    let mut st = super::snapshot_tests_trust::prompt_state(ThemeChoice::Dark, TrustScan::default(), false, Some("Continue rebase"));
    if let Some(Modal::Trust(p)) = &mut st.modal {
        p.unchecked = Some(Box::new(glowfly_model::effect::Accepted {
            at: None,
            reason: glowfly_core::trust::incoming::Unchecked::State("rebase-merge/git-rebase-todo".into()),
        }));
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("trust_prompt_state_unread");
    assert!(h.query_by_label_contains("couldn't read rebase-merge/git-rebase-todo of the rebase").is_some());
    assert!(h.query_by_label("Trust this repository?").is_some());
    assert!(h.query_by_label("Trust and run").is_some());
}
