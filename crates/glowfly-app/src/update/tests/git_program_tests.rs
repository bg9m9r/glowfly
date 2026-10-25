//! Headless tests for Preferences > Git: the scan when the page opens, checking a chosen git
//! before it is saved and used, refusals, Browse…, and the banner's offer of the PATH git when
//! the saved git can't run.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::effect::LfsEffect;
use crate::git_env::{GitEnvEffect, GitEnvMsg};
use crate::msg::{Msg, PrefsMsg};
use crate::state::{AppState, PrefPage};
use glowfly_core::git_program::Candidate;
use glowfly_core::git_version::{Feature, GitInfo};
use std::path::PathBuf;
use std::time::Instant;

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
    glowfly_core::paths::temp_dir().join("tools").join("git")
}

/// A saved git that is no longer there.
fn gone() -> PathBuf {
    glowfly_core::paths::temp_dir().join("gone").join("git")
}

fn check_path_gits(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::GitEnv(GitEnvEffect::CheckPathGit { .. }))).count()
}

fn detects(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::GitEnv(GitEnvEffect::Detect { .. })))
}

/// The saved git is gone, as the startup check finds; returns what that pushed.
fn saved_git_gone(st: &mut AppState) -> Vec<Effect> {
    st.settings.git_program = Some(gone().to_string_lossy().into_owned());
    run(st, GitEnvMsg::Detected { generation: 0, info: GitInfo { path: Some(gone()), version: None, chosen: true } })
}

/// The saved git is gone and the git on PATH works: the banner offers it.
fn offered(st: &mut AppState) {
    saved_git_gone(st);
    run(st, GitEnvMsg::PathGitChecked(Some(found(tools_git()))));
    assert!(st.git.path_offer().is_some());
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
    run(
        &mut st,
        GitEnvMsg::Detected {
            generation: 0,
            info: GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false },
        },
    );
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
    assert!(fx.iter().any(|e| matches!(e, Effect::Lfs(LfsEffect::Detect { .. }))), "another git may bring another git-lfs: {fx:?}");
    assert!(st.git.page.checking.is_none());
    let banner = st.git_banner().expect("a too-old banner, shown again");
    assert!(banner.contains("2.34.1"), "{banner}");
    assert!(!st.git.has(Feature::MergeTree), "gating follows at once");
    assert!(st.log.iter().any(|l| l.ok && l.text.starts_with("Glowfly now runs git 2.34.1 at ")), "logged");

    run(&mut st, GitEnvMsg::Choose(None));
    let info = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false };
    run(&mut st, GitEnvMsg::Switched { program: None, result: Ok(info) });
    assert_eq!(st.settings.git_program, None);
    assert_eq!(st.git_banner(), None);
    assert!(st.git.has(Feature::MergeTree));
}

#[test]
fn a_program_that_isnt_git_is_refused() {
    let mut st = state();
    let before = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false };
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: before.clone() });
    let p = tools_git();
    run(&mut st, GitEnvMsg::Choose(Some(p.clone())));
    // An answer for a program that isn't being checked is dropped.
    let other = glowfly_core::paths::temp_dir().join("other").join("git");
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
        GitEnvMsg::UsePathGit,
        GitEnvMsg::PathGitChecked(None),
    ] {
        assert!(m.on_inactive().is_none());
    }
}

#[test]
fn a_missing_saved_git_offers_the_path_git() {
    let mut st = state();
    let fx = saved_git_gone(&mut st);
    assert_eq!(check_path_gits(&fx), 1, "{fx:?}");
    assert!(st.git.path_offer().is_none(), "nothing to offer before the check answers");
    assert!(st.git_banner().is_some_and(|b| b.contains("chosen in Preferences > Git")));
    run(&mut st, GitEnvMsg::PathGitChecked(Some(found(tools_git()))));
    assert_eq!(st.git.path_offer(), Some((tools_git().as_path(), (2, 45, 1))));

    let fx = run(&mut st, GitEnvMsg::Rescan);
    let generation = discover_gen(&fx).expect("a scan");
    assert!(detects(&fx), "Look again checks the saved git again: {fx:?}");
    let other = glowfly_core::paths::temp_dir().join("other").join("git");
    let candidates = vec![
        Candidate { path: other.clone(), first_on_path: true, check: Ok((2, 46, 0)) },
        Candidate { first_on_path: false, ..found(tools_git()) },
    ];
    run(&mut st, GitEnvMsg::Discovered { generation, candidates });
    assert_eq!(st.git.path_offer(), Some((other.as_path(), (2, 46, 0))), "the scan is fresher");

    let fx = run(
        &mut st,
        GitEnvMsg::Detected { generation: 0, info: GitInfo { path: Some(tools_git()), version: Some((2, 45, 1)), chosen: false } },
    );
    assert_eq!(check_path_gits(&fx), 0, "{fx:?}");
    assert!(st.git.path_git.is_none() && st.git.path_offer().is_none());
}

#[test]
fn a_missing_saved_git_with_no_path_git_keeps_the_explanation() {
    let mut st = state();
    saved_git_gone(&mut st);
    run(&mut st, GitEnvMsg::PathGitChecked(None));
    assert!(st.git.path_offer().is_none());
    assert!(st.git_banner().is_some_and(|b| b.contains("Choose another there")));
    let broken = Candidate { check: Err("there is no file there".into()), ..found(tools_git()) };
    run(&mut st, GitEnvMsg::PathGitChecked(Some(broken)));
    assert!(st.git.path_offer().is_none(), "a PATH git that doesn't run isn't offered");
    let old = Candidate { check: Ok((2, 18, 0)), ..found(tools_git()) };
    run(&mut st, GitEnvMsg::PathGitChecked(Some(old)));
    assert!(st.git.path_offer().is_none(), "too old to offer");
    assert!(run(&mut st, GitEnvMsg::UsePathGit).is_empty(), "a stale offer does nothing");
    assert!(st.git.page.checking.is_none());

    // The saved git works again: a late answer is dropped.
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: GitInfo { path: Some(gone()), version: Some((2, 45, 0)), chosen: true } });
    run(&mut st, GitEnvMsg::PathGitChecked(Some(found(tools_git()))));
    assert!(st.git.path_git.is_none() && st.git.path_offer().is_none());
}

#[test]
fn the_banner_button_switches_to_the_path_git() {
    let mut st = state();
    offered(&mut st);
    run(&mut st, GitEnvMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Git));
    let toasts = st.toasts.len();
    let fx = run(&mut st, GitEnvMsg::UsePathGit);
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::Use { program: None, .. })]), "{fx:?}");
    assert!(st.git.page.checking.as_ref().is_some_and(|c| c.program.is_none() && c.from_banner));
    assert!(run(&mut st, GitEnvMsg::UsePathGit).is_empty(), "one check at a time");
    assert!(run(&mut st, GitEnvMsg::Choose(Some(tools_git()))).is_empty());

    let info = GitInfo { path: Some(tools_git()), version: Some((2, 45, 1)), chosen: false };
    let fx = run(&mut st, GitEnvMsg::Switched { program: None, result: Ok(info) });
    assert_eq!(st.settings.git_program, None, "git from your PATH, following later PATH changes");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.git_program.is_none())), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::Lfs(LfsEffect::Detect { .. }))), "{fx:?}");
    assert!(discover_gen(&fx).is_some(), "the open page shows the PATH git in use: {fx:?}");
    assert_eq!(st.git_banner(), None);
    assert!(st.git.path_offer().is_none() && st.git.path_git.is_none());
    assert_eq!(st.toasts.len(), toasts + 1);
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok && toast.text.starts_with("Glowfly now runs git 2.45.1 from your PATH"), "{}", toast.text);
    assert!(st.log.iter().any(|l| l.ok && l.text.starts_with("Glowfly now runs git 2.45.1 at ")), "logged as before");

    // A switch made on the page isn't toasted.
    let toasts = st.toasts.len();
    run(&mut st, GitEnvMsg::Choose(None));
    let info = GitInfo { path: Some(tools_git()), version: Some((2, 45, 1)), chosen: false };
    run(&mut st, GitEnvMsg::Switched { program: None, result: Ok(info) });
    assert_eq!(st.toasts.len(), toasts);
}

#[test]
fn rescan_without_a_saved_git_only_scans() {
    let mut st = state();
    let fx = run(&mut st, GitEnvMsg::Rescan);
    assert!(discover_gen(&fx).is_some() && !detects(&fx), "{fx:?}");

    let old = GitInfo { path: Some(tools_git()), version: Some((2, 34, 1)), chosen: false };
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: old.clone() });
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: old });
    let logged = st.log.iter().filter(|l| l.text.starts_with("Your git is 2.34.1")).count();
    assert_eq!(logged, 1, "an unchanged answer isn't logged again");

    // A changed answer shows the banner again after a Dismiss.
    run(&mut st, GitEnvMsg::DismissBanner);
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: GitInfo { path: Some(tools_git()), version: None, chosen: false } });
    assert!(st.git_banner().is_some());
}

#[test]
fn a_check_started_before_a_switch_is_dropped() {
    let mut st = state();
    offered(&mut st);
    // Look again checks the saved git again; that check can take seconds.
    let fx = run(&mut st, GitEnvMsg::Rescan);
    let generation = fx
        .iter()
        .find_map(|e| match e {
            Effect::GitEnv(GitEnvEffect::Detect { generation, .. }) => Some(*generation),
            _ => None,
        })
        .expect("a check of the saved git");
    // Meanwhile the banner's button switches to the PATH git.
    run(&mut st, GitEnvMsg::UsePathGit);
    let path_git = GitInfo { path: Some(tools_git()), version: Some((2, 45, 1)), chosen: false };
    run(&mut st, GitEnvMsg::Switched { program: None, result: Ok(path_git.clone()) });
    assert_eq!(st.git_banner(), None);

    let stale = GitInfo { path: Some(gone()), version: None, chosen: true };
    let fx = run(&mut st, GitEnvMsg::Detected { generation, info: stale.clone() });
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.git.info.as_ref(), Some(&path_git), "the PATH git stays in use");
    assert_eq!(st.git_banner(), None);
    assert!(st.git.has(Feature::MergeTree));

    // A check started after the switch is applied.
    let fx = run(&mut st, GitEnvMsg::Detect);
    let Some(Effect::GitEnv(GitEnvEffect::Detect { generation, .. })) = fx.first() else { panic!("{fx:?}") };
    let newer = GitInfo { version: Some((2, 46, 0)), ..path_git };
    run(&mut st, GitEnvMsg::Detected { generation: *generation, info: newer.clone() });
    assert_eq!(st.git.info.as_ref(), Some(&newer));
}
