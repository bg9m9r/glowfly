//! What the runner reports about saving recent.txt and tabs.txt (update/recent_files.rs): a
//! late-read recent list merged into the Recent list, a kept copy logged, and refused saves
//! toasted, in each language.

use super::tests::state;
use super::update;
use crate::effect::{Effect, SessionEffect};
use crate::msg::{Msg, RecentMsg, SaveNotes, StateFile};
use crate::state::AppState;
use crate::state::repo_lists::RECENT_LIMIT;
use glowfly_core::i18n::Lang;
use std::path::PathBuf;
use std::time::Instant;

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn saved(st: &mut AppState, file: StateFile, notes: SaveNotes) -> Vec<Effect> {
    update(st, Msg::Recent(RecentMsg::StateFileSaved { file, notes }), Instant::now())
}

#[test]
fn read_late_recent_is_merged_and_capped() {
    let mut st = state();
    st.recent = paths(&["/w/x"]);
    let fx = saved(&mut st, StateFile::Recent, SaveNotes { read_late: Some(paths(&["/w/x", "/w/a", "/w/b"])), kept: None });
    assert_eq!(st.recent, paths(&["/w/x", "/w/a", "/w/b"]));
    assert!(fx.iter().any(|e| matches!(e, Effect::Session(SessionEffect::CheckRepoFolders(_)))), "{fx:?}");
    let t = st.settings.lang();
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == t.tr("repos.recent_file.read_late.log")));
    // Eleven of the session's and five late ones: the list stays at its limit, the session's first.
    let mut st = state();
    st.recent = (0..11).map(|k| PathBuf::from(format!("/w/s{k}"))).collect();
    let late: Vec<PathBuf> = (0..5).map(|k| PathBuf::from(format!("/w/late{k}"))).collect();
    saved(&mut st, StateFile::Recent, SaveNotes { read_late: Some(late.clone()), kept: None });
    assert_eq!(st.recent.len(), RECENT_LIMIT);
    assert_eq!(st.recent[11], late[0]);
}

#[test]
fn read_late_tabs_are_only_logged() {
    let mut st = state();
    st.recent = paths(&["/w/x"]);
    let fx = saved(&mut st, StateFile::Tabs, SaveNotes { read_late: Some(paths(&["/w/a"])), kept: None });
    assert_eq!(st.recent, paths(&["/w/x"]));
    assert!(fx.is_empty(), "{fx:?}");
    let t = st.settings.lang();
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == t.tr("tabs.saved_file.read_late.log")));
}

#[test]
fn kept_copy_is_logged_with_path_and_count() {
    let mut st = state();
    let backup = glowfly_core::paths::temp_dir().join("tabs.txt.bak");
    let fx = saved(&mut st, StateFile::Tabs, SaveNotes { read_late: None, kept: Some((backup.clone(), 3)) });
    assert!(fx.is_empty(), "{fx:?}");
    let t = st.settings.lang();
    let want = t.trn("tabs.saved_file.kept.log", 3, &[("path", &glowfly_core::native_path::shown(&backup))]);
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == want), "{:?}", st.log.last().map(|l| &l.text));
    assert!(want.contains('3') && want.contains("tabs.txt.bak"), "{want}");
    assert!(st.toasts.is_empty(), "no toast for a copy kept");
}

#[test]
fn blocked_saves_toast_and_log() {
    let cases = [
        (StateFile::Recent, true, "repos.clear_recent.blocked.toast"),
        (StateFile::Recent, false, "repos.recent_file.blocked.toast"),
        (StateFile::Tabs, false, "tabs.saved_file.blocked.toast"),
        (StateFile::Tabs, true, "tabs.saved_file.blocked.toast"),
    ];
    for lang in [Lang::En, Lang::De] {
        for (file, clearing, key) in cases {
            let mut st = state();
            st.settings.language = lang.code().into();
            let msg = Msg::Recent(RecentMsg::StateFileBlocked { file, clearing, detail: "d".into() });
            update(&mut st, msg, Instant::now());
            let want = lang.trf(key, &[("detail", &"d")]);
            assert!(want.contains("(d)"), "{want}");
            assert!(st.toasts.iter().any(|t| !t.ok && t.text == want), "{key} in {}", lang.code());
            assert!(st.log.last().is_some_and(|l| !l.ok && l.text == want), "{key} in {}", lang.code());
            if lang == Lang::De {
                assert_ne!(want, Lang::En.trf(key, &[("detail", &"d")]), "{key} is translated");
            }
        }
    }
}
