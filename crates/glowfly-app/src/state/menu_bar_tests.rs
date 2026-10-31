//! The menu bar's texts: Window menu key hints built from the keymap, zoom limits, the links and
//! the copied version info.

use super::*;
use crate::state::git_env::GitState;
use glowfly_core::git_version::GitInfo;

/// The key text the Window menu shows for tab `i` of `len`.
fn tab_hint(keys: &Keymap, i: usize, len: usize) -> Option<String> {
    tab_chord(keys, i, len).map(|c| c.text())
}

#[test]
fn tab_chords_follow_the_binding_they_name() {
    let keys = Keymap::default();
    for len in [9, 12] {
        for i in 0..12 {
            let first = tab_binding(i, len).and_then(|b| keys.chords(b).first().copied());
            let want = if i < 8 { first.map(|c| Chord { key: KeyName::Digit(i as u8 + 1), ..c }) } else { first };
            assert_eq!(tab_chord(&keys, i, len), want, "tab {i} of {len}");
        }
    }
    assert_eq!(tab_binding(8, 9), Some(BindingId::LastTab));
    assert_eq!(tab_binding(8, 12), None);
}

#[test]
fn tab_hints_come_from_the_go_to_tab_keys() {
    let mut keys = Keymap::default();
    let digit = |n: u8| Chord { key: KeyName::Digit(n), ..keys.chords(BindingId::GoToTab)[0] }.text();
    assert_eq!(tab_hint(&keys, 2, 5), Some(Chord::cmd(KeyName::Digit(3)).text()));
    assert_eq!(tab_hint(&keys, 0, 1), Some(digit(1)));
    assert_eq!(tab_hint(&keys, 7, 12), Some(digit(8)));
    // Beyond the eighth: only a last tab has a key.
    assert_eq!(tab_hint(&keys, 8, 9).as_deref(), keys.hint(BindingId::LastTab));
    assert!(tab_hint(&keys, 8, 9).is_some());
    assert_eq!(tab_hint(&keys, 8, 12), None);
    keys.set(BindingId::LastTab, Vec::new());
    assert_eq!(tab_hint(&keys, 8, 9), None);
}

#[test]
fn zoom_items_stop_at_the_limits() {
    let max = ZOOM_STEPS[ZOOM_STEPS.len() - 1];
    let min = ZOOM_STEPS[0];
    assert_eq!(zoom_blocked(Lang::En, DEFAULT_ZOOM_PERCENT), [None, None, Some("Already at 100%".to_owned())]);
    assert_eq!(zoom_blocked(Lang::En, max), [Some("Already at the largest zoom (200%)".to_owned()), None, None]);
    assert_eq!(zoom_blocked(Lang::En, min), [None, Some("Already at the smallest zoom (50%)".to_owned()), None]);
    assert_eq!(zoom_blocked(Lang::En, 125), [None, None, None]);
}

#[test]
fn zoom_limits_follow_the_language() {
    let max = ZOOM_STEPS[ZOOM_STEPS.len() - 1];
    let de = zoom_blocked(Lang::De, max);
    assert_eq!(de[0], Some(Lang::De.trf("menu.view.zoom_max", &[("percent", &max)])));
    assert_ne!(de[0], zoom_blocked(Lang::En, max)[0]);
}

#[test]
fn a_new_tab_is_named_in_the_language() {
    let s = glowfly_core::settings::Settings { language: "de".into(), ..Default::default() };
    let mut st = AppState::new(s, Vec::new(), std::time::Instant::now());
    let dir = glowfly_core::paths::temp_dir().join("glowfly-tab-name");
    st.tabs = vec![crate::state::Tab::empty(), crate::state::Tab::unloaded(dir)];
    st.active_tab = 0;
    assert_eq!(tab_name(&st, 0), Lang::De.tr("tabs.new_tab.name"));
    assert_ne!(tab_name(&st, 0), "New tab");
    assert_eq!(st.tabs[0].name, "New tab", "the stored placeholder stays English");
    assert_eq!(tab_name(&st, 1), "glowfly-tab-name");
}

#[test]
fn reflow_joins_paragraph_lines() {
    let text = "Permission is hereby granted,\nfree of charge.\n\nConditions:\n\n1. Keep this\n   notice.\n2. Don't sue.\n- also this\n";
    let want = "Permission is hereby granted, free of charge.\n\nConditions:\n\n1. Keep this\n   notice.\n2. Don't sue.\n- also this";
    assert_eq!(reflow(text), want);
    assert_eq!(reflow(&text.replace('\n', "\r\n")), want);
    assert_eq!(reflow("a\nb\n"), "a b");
    assert_eq!(reflow(""), "");
}

#[test]
fn links_point_at_the_project() {
    assert!(WEBSITE.starts_with("https://"));
    assert!(ISSUES.starts_with(WEBSITE) && ISSUES.ends_with("/issues"));
}

#[test]
fn version_info_names_glowfly_and_git() {
    let mut git = GitState::default();
    assert_eq!(version_info(PLACEHOLDER_VERSION, &git), format!("Glowfly {PLACEHOLDER_VERSION}"));
    assert_eq!(git_line(&git), None);
    git.info = Some(GitInfo { path: None, version: None, chosen: false });
    assert_eq!(git_line(&git), None, "git didn't say its version");
    git.info = Some(GitInfo { path: None, version: Some((2, 47, 1)), chosen: false });
    assert_eq!(git_line(&git).as_deref(), Some("Git 2.47.1"));
    assert_eq!(version_info(PLACEHOLDER_VERSION, &git), format!("Glowfly {PLACEHOLDER_VERSION}\nGit 2.47.1"));
    let program = glowfly_core::paths::temp_dir().join("git");
    git.info = Some(GitInfo { path: Some(program.clone()), version: Some((2, 47, 1)), chosen: true });
    let shown = glowfly_core::native_path::shown(&program);
    assert_eq!(version_info(PLACEHOLDER_VERSION, &git), format!("Glowfly {PLACEHOLDER_VERSION}\nGit 2.47.1 ({shown})"));
}
