//! Headless tests for Preferences > Git: the scan when the page opens, checking a chosen git
//! before it is saved and used, refusals, and Browse….

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::git_env::{GitEnvEffect, GitEnvMsg};
use crate::msg::{Msg, PrefsMsg};
use crate::state::{AppState, PrefPage};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::git_program::Candidate;
use trellis_core::git_version::{Feature, GitInfo};

fn run(st: &mut AppState, m: GitEnvMsg) -> Vec<Effect> {
    update(st, Msg::GitEnv(m), Instant::now())
}

fn discover_gen(fx: &[Effect]) -> Option<u64> {
    fx.iter().find_map(|e| match e {
        Effect::GitEnv(GitEnvEffect::Discover { generation, .. }) => Some(*generation),
        _ => None,
    })
}

fn tools_git() -> PathBuf {
    trellis_core::paths::temp_dir().join("tools").join("git")
}

fn found(path: PathBuf) -> Candidate {
    Candidate { path, first_on_path: true, check: Ok((2, 45, 1)) }
}

#[test]
fn the_git_page_scans_when_it_opens() {
    let mut st = state();
    let fx = run(&mut st, GitEnvMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Git));
    assert_eq!(discover_gen(&fx), Some(1), "{fx:?}");
    assert!(st.git.page.candidates.is_none(), "scanning");

    // Already open on another page: switches and scans again.
    st.prefs.as_mut().unwrap().page = PrefPage::General;
    let fx = run(&mut st, GitEnvMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Git));
    assert_eq!(discover_gen(&fx), Some(2));

    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Git)), Instant::now());
    assert_eq!(discover_gen(&fx), Some(3), "opening Preferences on the page scans too");

    run(&mut st, GitEnvMsg::Discovered { generation: 2, candidates: vec![found(tools_git())] });
    assert!(st.git.page.candidates.is_none(), "a stale scan is dropped");
    run(&mut st, GitEnvMsg::Discovered { generation: 3, candidates: vec![found(tools_git())] });
    assert_eq!(st.git.page.path_git().map(|c| c.path.clone()), Some(tools_git()));

    let fx = run(&mut st, GitEnvMsg::Rescan);
    assert_eq!(discover_gen(&fx), Some(4));
    assert!(st.git.page.candidates.is_none(), "Look again clears the list");
}

#[test]
fn choosing_a_git_checks_then_saves_and_redetects() {
    let mut st = state();
    run(&mut st, GitEnvMsg::Detected(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false }));
    let p = tools_git();
    let fx = run(&mut st, GitEnvMsg::Choose(Some(p.clone())));
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::Use { program: Some(q), .. })] if *q == p), "{fx:?}");
    assert_eq!(st.settings.git_program, None, "not saved before the check");
    assert!(st.git.page.checking.is_some());
    assert!(run(&mut st, GitEnvMsg::Choose(None)).is_empty(), "one check at a time");
    assert!(run(&mut st, GitEnvMsg::Browse).is_empty(), "no Browse… while checking");

    st.git.banner_dismissed = true;
    let info = GitInfo { path: Some(p.clone()), version: Some((2, 34, 1)), chosen: true };
    let fx = run(&mut st, GitEnvMsg::Switched { program: Some(p.clone()), result: Ok(info) });
    assert_eq!(st.settings.git_program.as_deref(), Some(p.to_str().unwrap()));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.git_program == st.settings.git_program)), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::DetectLfs { .. })), "another git may bring another git-lfs: {fx:?}");
    assert!(st.git.page.checking.is_none());
    let banner = st.git.banner().expect("a too-old banner, shown again");
    assert!(banner.contains("2.34.1"), "{banner}");
    assert!(!st.git.has(Feature::MergeTree), "gating follows at once");
    assert!(st.log.iter().any(|l| l.ok && l.text.starts_with("Trellis now runs git 2.34.1 at ")), "logged");

    run(&mut st, GitEnvMsg::Choose(None));
    let info = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false };
    run(&mut st, GitEnvMsg::Switched { program: None, result: Ok(info) });
    assert_eq!(st.settings.git_program, None);
    assert_eq!(st.git.banner(), None);
    assert!(st.git.has(Feature::MergeTree));
}

#[test]
fn a_program_that_isnt_git_is_refused() {
    let mut st = state();
    let before = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false };
    run(&mut st, GitEnvMsg::Detected(before.clone()));
    let p = tools_git();
    run(&mut st, GitEnvMsg::Choose(Some(p.clone())));
    // An answer for a program that isn't being checked is dropped.
    let other = trellis_core::paths::temp_dir().join("other").join("git");
    let fx = run(&mut st, GitEnvMsg::Switched { program: Some(other), result: Ok(GitInfo::default()) });
    assert!(fx.is_empty() && st.git.page.checking.is_some() && st.git.info.as_ref() == Some(&before));

    let why = "it isn't a working git: it printed \"hello\"".to_owned();
    let fx = run(&mut st, GitEnvMsg::Switched { program: Some(p.clone()), result: Err(why.clone()) });
    assert!(!fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "{fx:?}");
    assert_eq!(st.settings.git_program, None);
    assert_eq!(st.git.info.as_ref(), Some(&before), "the info is unchanged");
    assert!(st.git.page.checking.is_none());
    assert_eq!(st.git.page.refusal.as_ref().map(|r| r.1.clone()), Some(why.clone()));
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.contains(&why)));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.starts_with("Can't use ")));
    run(&mut st, GitEnvMsg::Rescan);
    assert!(st.git.page.refusal.is_none(), "a new scan clears it");
}

#[test]
fn browse_picks_and_checks() {
    let mut st = state();
    let fx = run(&mut st, GitEnvMsg::Browse);
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::PickProgram)]), "{fx:?}");
    assert!(run(&mut st, GitEnvMsg::Picked(None)).is_empty(), "cancelled");
    let fx = run(&mut st, GitEnvMsg::Picked(Some(tools_git())));
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::Use { program: Some(_), .. })]), "{fx:?}");
}

#[test]
#[cfg(unix)]
fn a_path_that_isnt_unicode_is_refused() {
    use std::os::unix::ffi::OsStringExt;
    let mut st = state();
    let p = PathBuf::from(std::ffi::OsString::from_vec(b"/opt/g\xffit/git".to_vec()));
    let fx = run(&mut st, GitEnvMsg::Picked(Some(p)));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.git.page.refusal.as_ref().is_some_and(|r| r.1.contains("valid Unicode")));
}

#[test]
fn choosing_a_git_is_the_same_for_every_tab() {
    let p = tools_git();
    for m in [
        GitEnvMsg::ShowPage,
        GitEnvMsg::Rescan,
        GitEnvMsg::Choose(Some(p.clone())),
        GitEnvMsg::Browse,
        GitEnvMsg::Discovered { generation: 1, candidates: Vec::new() },
        GitEnvMsg::Picked(Some(p.clone())),
        GitEnvMsg::Switched { program: None, result: Err("no".into()) },
    ] {
        assert!(m.on_inactive().is_none());
    }
}
