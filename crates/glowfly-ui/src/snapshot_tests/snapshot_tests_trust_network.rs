//! The trust prompt for a repository whose own config changes how git connects (TLS checks
//! and proxies), listed apart from the settings that run commands, and git-lfs's programs
//! among those.

use super::snapshot_tests::harness;
use super::snapshot_tests_trust::{setting, with_prompt};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::trust::TrustScan;

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
    assert!(h.query_by_label("Settings that change how git connects").is_some());
    assert!(h.query_by_label("http.sslVerify = false").is_some());
    assert!(h.query_by_label_contains("turn off certificate checks").is_some());
}
