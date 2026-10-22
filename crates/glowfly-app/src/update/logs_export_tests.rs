//! Headless tests for Export logs: the save dialog, the bundle's header and the answers.

use super::super::tests::state;
use super::super::update;
use crate::effect::{Effect, OsEffect};
use crate::logs::{LogsEffect, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_bar::VERSION;
use glowfly_core::git_version::GitInfo;
use glowfly_core::logs::{BundleReport, bundle_name};
use glowfly_core::native_path;
use std::path::PathBuf;
use std::time::Instant;

fn logs(st: &mut AppState, m: LogsMsg) -> Vec<Effect> {
    update(st, Msg::Logs(m), Instant::now())
}

fn dest() -> PathBuf {
    glowfly_core::paths::temp_dir().join("glowfly-export").join("bundle.txt")
}

fn value<'a>(header: &'a [(String, String)], label: &str) -> Option<&'a str> {
    header.iter().find(|(l, _)| l == label).map(|(_, v)| v.as_str())
}

fn export_header(st: &mut AppState) -> Vec<(String, String)> {
    let fx = logs(st, LogsMsg::ExportTo(Some(dest())));
    match fx.as_slice() {
        [Effect::Logs(LogsEffect::Export { dest: d, header })] if *d == dest() => header.clone(),
        other => panic!("not an export: {other:?}"),
    }
}

#[test]
fn export_asks_where_to_save() {
    let mut st = state();
    st.view.now = 1_772_600_767;
    st.view.utc_offset = 7200;
    let fx = logs(&mut st, LogsMsg::Export);
    match fx.as_slice() {
        [Effect::Logs(LogsEffect::PickExport { name })] => assert_eq!(name, &bundle_name(1_772_600_767, 7200)),
        other => panic!("{other:?}"),
    }
    assert!(logs(&mut st, LogsMsg::ExportTo(None)).is_empty(), "cancelled");
}

#[test]
fn export_to_sends_the_header() {
    let mut st = state();
    st.view.now = 1_772_600_767;
    st.git.info = None;
    let h = export_header(&mut st);
    let labels: Vec<&str> = h.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels[..5], ["Exported", "Glowfly", "OS", "Git", "Extended logging"]);
    assert_eq!(value(&h, "Exported"), Some("2026-03-04 05:06:07 +00:00"));
    assert_eq!(value(&h, "Glowfly"), Some(VERSION));
    assert_eq!(value(&h, "OS"), Some(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH).as_str()));
    assert_eq!(value(&h, "Git"), Some("not checked yet"));
    assert_eq!(value(&h, "Extended logging"), Some("off"));

    st.git.info = Some(GitInfo { path: None, version: None, chosen: false });
    assert_eq!(value(&export_header(&mut st), "Git"), Some("not found"));
    let git = glowfly_core::paths::temp_dir().join("git").join("git");
    st.git.info = Some(GitInfo { path: Some(git.clone()), version: Some((2, 47, 1)), chosen: false });
    let want = format!("2.47.1 ({})", native_path::shown(&git));
    assert_eq!(value(&export_header(&mut st), "Git"), Some(want.as_str()));
    st.git.info = Some(GitInfo { path: None, version: Some((2, 47, 1)), chosen: false });
    assert_eq!(value(&export_header(&mut st), "Git"), Some("2.47.1"));

    st.settings.extended_logging = true;
    assert_eq!(value(&export_header(&mut st), "Extended logging"), Some("on"));
}

#[test]
fn exported_ok_and_failed() {
    let mut st = state();
    let report = BundleReport { files: 4, bytes: 1234 };
    logs(&mut st, LogsMsg::Exported { dest: dest(), result: Ok(report) });
    let t = st.toasts.last().expect("a toast");
    assert!(t.ok);
    assert_eq!(t.text, "Exported the logs to bundle.txt (4 files)");
    let action = t.action.as_ref().expect("show in folder");
    assert_eq!(action.label, crate::state::files::show_in_folder(st.settings.lang()));
    assert!(matches!(&*action.msg, Msg::Logs(LogsMsg::ShowExported(p)) if *p == dest()));
    let logged = format!("Exported the logs to {}", native_path::shown(&dest()));
    assert!(st.log.iter().any(|e| e.ok && e.text == logged), "logged in the activity");

    let fx = logs(&mut st, LogsMsg::ShowExported(dest()));
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::RevealPath(p))] if *p == dest()), "{fx:?}");

    logs(&mut st, LogsMsg::Exported { dest: dest(), result: Err("disk full".into()) });
    let t = st.toasts.last().expect("a toast");
    assert!(!t.ok);
    assert_eq!(t.text, "Couldn't export the logs: disk full");
    assert!(t.action.as_ref().is_some_and(|a| a.label == "Details"), "announced with Details");
    assert!(st.log.iter().any(|e| !e.ok && e.text == "Couldn't export the logs: disk full"));
}
