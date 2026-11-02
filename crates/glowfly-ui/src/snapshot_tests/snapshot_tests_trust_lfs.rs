//! The trust prompt's marks: Git LFS's standard settings and hooks with a tick and "Git LFS",
//! everything else with a warning triangle, the program word of each setting emphasised, the
//! long path cut to one line, and Trust and Trust and run as plain (unfilled) buttons.

use super::snapshot_tests::harness;
use super::snapshot_tests_painted::fills_inside;
use super::snapshot_tests_trust::{setting, with_prompt};
use crate::state::AppState;
use crate::state::Modal;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::color;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::trust::TrustScan;
use glowfly_core::trust::known::Known;
use std::path::{Path, PathBuf};

const WORKDIR: &str = "C:/Users/developer/source/repos/clients/acme-corporation/platform/backend/services/tools";

/// The prompt for a Git LFS repository with a deploy key and its own pre-commit hook.
fn lfs_prompt(theme: ThemeChoice) -> AppState {
    let known = ["post-checkout", "post-commit", "post-merge"].map(|h| (h.to_owned(), Known::GitLfs));
    let scan = TrustScan {
        config: vec![
            setting("filter.lfs.clean", "git-lfs clean -- %f"),
            setting("filter.lfs.smudge", "git-lfs smudge -- %f"),
            setting("filter.lfs.process", "git-lfs filter-process"),
            setting("core.sshCommand", "ssh -i ~/.ssh/deploy_key"),
        ],
        hooks: vec!["post-checkout".into(), "post-commit".into(), "post-merge".into(), "pre-commit".into()],
        known_hooks: known.into_iter().collect(),
        hooks_dir: Some(PathBuf::from(format!("{WORKDIR}/.git/hooks"))),
        ..TrustScan::default()
    };
    let mut st = with_prompt(theme, scan);
    if let Some(Modal::Trust(p)) = &mut st.modal {
        p.workdir = PathBuf::from(WORKDIR);
        p.real = PathBuf::from(WORKDIR);
    }
    st
}

#[test]
fn trust_prompt_lfs() {
    let mut h = harness(lfs_prompt(ThemeChoice::Graphite), true);
    h.run();
    h.check_snapshot("trust_prompt_lfs");
    assert_eq!(h.query_all_by_label("Git LFS").count(), 6, "three settings and three hooks are Git LFS's");
    assert_eq!(h.query_all_by_label("Not recognised").count(), 2, "the deploy key and pre-commit");
    assert!(h.query_by_label("filter.lfs.clean = git-lfs clean -- %f").is_some());
    let whole = glowfly_core::native_path::shown(Path::new(WORKDIR));
    h.get_by_label(&whole).hover();
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(h.query_all_by_label(&whole).count(), 2, "the cut path's tooltip has the whole path");
}

#[test]
fn trust_prompt_lfs_light() {
    let mut h = harness(lfs_prompt(ThemeChoice::Porcelain), false);
    h.run();
    h.check_snapshot("trust_prompt_lfs_light");
}

#[test]
fn neither_trust_answer_is_a_filled_button() {
    for held in [None, Some("Pull")] {
        let mut st = lfs_prompt(ThemeChoice::Graphite);
        if let Some(Modal::Trust(p)) = &mut st.modal {
            p.held = held.map(str::to_owned);
        }
        let mut h = harness(st, true);
        h.run();
        let trust = if held.is_some() { "Trust and run" } else { "Trust" };
        assert!(h.query_by_label(trust).is_some());
        let fills = fills_inside(&h, h.ctx.screen_rect());
        assert!(!fills.contains(&color::primary()), "{trust}: no filled primary in the trust dialog");
    }
}
