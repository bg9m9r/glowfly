//! Repository trust: a job that runs in another repository than the active tab's (Publish to
//! GitHub finishing after the user switched tabs) is gated by that repository's own decision.

use super::tests::{scratch_repo, state};
use super::trust_tests::{asking, drive, open, run, untrusted_repo};
use crate::effect::Effect;
use crate::msg::{Msg, PublishMsg, TabsMsg, TrustMsg};
use crate::state::trust::{record, recorded};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use trellis_core::forge::CreatedRepo;
use trellis_core::paths::same_spelling;
use trellis_core::settings::RepoTrust;

const LABEL: &str = "Publish to GitHub";

/// Undecided repository A in the first tab (its prompt dismissed), then repository B, which
/// the user trusts, opened in a second tab and active.
fn a_then_b(name: &str) -> (AppState, PathBuf, PathBuf) {
    let a = untrusted_repo(&format!("{name}-a"));
    let mut st = state();
    open(&mut st, &a);
    drive(&mut st, Msg::CloseModal);
    let b = untrusted_repo(&format!("{name}-b"));
    record(&mut st.settings, &trellis_core::paths::resolve(&b), RepoTrust::Trusted);
    open(&mut st, &b);
    assert_eq!(st.active_tab, 1);
    assert!(!asking(&st) && !st.held(), "B is trusted");
    (st, a, b)
}

fn published(workdir: &Path) -> Msg {
    let created = CreatedRepo {
        full_name: "ada/demo".into(),
        clone_url: "https://example.invalid/ada/demo.git".into(),
        html_url: "https://example.invalid/ada/demo".into(),
    };
    Msg::Publish(PublishMsg::Published { workdir: workdir.to_path_buf(), branch: "main".into(), result: Ok(created) })
}

fn run_git_cwds(fx: &[Effect]) -> Vec<PathBuf> {
    fx.iter().filter_map(|e| if let Effect::RunGit { cwd, .. } = e { Some(cwd.clone()) } else { None }).collect()
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_job_for_an_undecided_repository_in_another_tab_asks_about_that_one() {
    let (mut st, a, b) = a_then_b("elsewhere-ask");
    let fx = run(&mut st, published(&a));
    assert!(run_git_cwds(&fx).is_empty(), "nothing runs yet: {fx:?}");
    let Some(Modal::Trust(p)) = &st.modal else { panic!("A's prompt: {:?}", st.log.last().map(|l| &l.text)) };
    assert_eq!(p.held.as_deref(), Some(LABEL));
    assert!(same_spelling(&p.workdir, &a) || same_spelling(&p.real, &trellis_core::paths::resolve(&a)), "{:?}", p.workdir);
    assert!(!p.scan.is_empty(), "A's own scan");
    let real_a = p.real.clone();

    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real: real_a.clone(), trusted: true }));
    assert_eq!(recorded(&st.settings, &real_a), Some(RepoTrust::Trusted));
    assert!(st.settings.saved_trust(&real_a).and_then(|t| t.print).is_some(), "with the print of what was shown");
    let cwds = run_git_cwds(&fx);
    assert_eq!(cwds.len(), 1, "{fx:?}");
    assert!(same_spelling(&cwds[0], &a), "{cwds:?}");
    let job = st.job.as_ref().expect("it runs in A while B is active");
    assert_eq!(job.label, LABEL);
    assert!(job.workdir.as_ref().is_some_and(|w| same_spelling(w, &a)), "{:?}", job.workdir);
    assert_eq!(st.active_tab, 1);
    assert!(st.log.iter().any(|l| l.text.ends_with("is trusted.")));
    cleanup(&[&a, &b]);
}

#[test]
fn cancel_runs_nothing_in_the_other_repository() {
    let (mut st, a, b) = a_then_b("elsewhere-cancel");
    run(&mut st, published(&a));
    assert!(st.trust.held.as_ref().is_some_and(|h| h.elsewhere));
    let fx = run(&mut st, Msg::CloseModal);
    assert!(run_git_cwds(&fx).is_empty() && st.job.is_none() && st.trust.held.is_none());
    assert!(st.log.last().is_some_and(|l| l.text.starts_with("Publish to GitHub wasn't run")), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.settings.saved_trust(&trellis_core::paths::resolve(&a)).is_none(), "nothing recorded");
    cleanup(&[&a, &b]);
}

#[test]
fn a_trusted_repository_in_another_tab_runs_at_once() {
    // A is trusted and B, active, isn't: A's decision is the one that counts.
    let a = untrusted_repo("elsewhere-trusted-a");
    let mut st = state();
    record(&mut st.settings, &trellis_core::paths::resolve(&a), RepoTrust::Trusted);
    open(&mut st, &a);
    let b = untrusted_repo("elsewhere-trusted-b");
    open(&mut st, &b);
    drive(&mut st, Msg::CloseModal);
    assert!(st.held());
    let fx = run(&mut st, published(&a));
    assert_eq!(run_git_cwds(&fx).len(), 1, "{fx:?}");
    assert!(!matches!(st.modal, Some(Modal::Trust(_))));
    assert!(st.job.as_ref().and_then(|j| j.workdir.as_ref()).is_some_and(|w| same_spelling(w, &a)));
    cleanup(&[&a, &b]);
}

#[test]
fn a_repository_open_in_no_tab_runs_only_when_trusted() {
    let a = untrusted_repo("elsewhere-closed-a");
    let b = scratch_repo("elsewhere-closed-b");
    let mut st = state();
    open(&mut st, &b);
    let msg_log = |st: &AppState| st.log.last().map(|l| l.text.clone()).unwrap_or_default();

    let fx = run(&mut st, published(&a));
    assert!(run_git_cwds(&fx).is_empty() && st.job.is_none() && !asking(&st));
    let text = msg_log(&st);
    assert!(text.starts_with("Publish to GitHub wasn't run:") && text.contains("isn't open, so Trellis can't check"), "{text}");
    assert!(text.ends_with("Open it and start it again."), "{text}");

    record(&mut st.settings, &a, RepoTrust::NotTrusted);
    let fx = run(&mut st, published(&a));
    assert!(run_git_cwds(&fx).is_empty() && st.job.is_none());
    assert!(msg_log(&st).contains("you chose not to trust"), "{}", msg_log(&st));

    record(&mut st.settings, &a, RepoTrust::Trusted);
    let fx = run(&mut st, published(&a));
    assert_eq!(run_git_cwds(&fx).len(), 1, "{fx:?}");
    cleanup(&[&a, &b]);
}

#[test]
fn closing_the_tab_leaves_its_saved_decision() {
    let (mut st, a, b) = a_then_b("elsewhere-tab-closed");
    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    assert_eq!(st.tabs.len(), 1);
    let fx = run(&mut st, published(&a));
    assert!(run_git_cwds(&fx).is_empty() && !asking(&st), "undecided and open nowhere: refused");
    cleanup(&[&a, &b]);
}
