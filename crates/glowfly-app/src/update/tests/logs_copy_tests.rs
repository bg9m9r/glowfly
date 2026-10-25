//! Headless tests for Copy entry and Copy log.

use super::super::tests::state;
use super::super::update;
use crate::effect::{Effect, UiEffect};
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::activity_copy::{activity_text, entry_text};
use crate::state::logs::{EntryId, LogEntry, LogFileView};
use glowfly_core::i18n::Lang;
use glowfly_core::logs::{LogTail, RunInfo};
use std::time::Instant;

fn logs(st: &mut AppState, m: LogsMsg) -> Vec<Effect> {
    update(st, Msg::Logs(m), Instant::now())
}

fn copied(fx: &[Effect]) -> Vec<&str> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ui(UiEffect::CopyText(t)) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

fn push_run(st: &mut AppState) -> EntryId {
    let run = RunInfo { title: "Push".into(), summary: "Everything up-to-date".into(), took_ms: Some(1250), routine: false };
    let e = LogEntry { at: 1_772_600_767, run: Some(run), ..LogEntry::new(true, "Push done\n$ git push\nEverything up-to-date") };
    let id = e.id;
    st.log.push(e);
    id
}

#[test]
fn copy_entry_puts_the_entry_on_the_clipboard() {
    let mut st = state();
    st.view.utc_offset = 3600;
    let id = push_run(&mut st);
    let toasts = st.toasts.len();
    let fx = logs(&mut st, LogsMsg::CopyEntry(id));
    let want = entry_text(st.log.iter().find(|e| e.id == id).unwrap(), Lang::En, 3600);
    assert_eq!(copied(&fx), [want.as_str()]);
    assert_eq!(fx.len(), 1, "{fx:?}");
    assert_eq!(st.toasts.len(), toasts, "no toast, like the other Copy buttons");

    st.log.clear();
    assert!(logs(&mut st, LogsMsg::CopyEntry(id)).is_empty(), "a cleared entry copies nothing");
    assert!(logs(&mut st, LogsMsg::CopyEntry(EntryId::next())).is_empty());
}

#[test]
fn copy_log_activity_and_file_tabs() {
    let mut st = state();
    st.log.clear();
    assert!(logs(&mut st, LogsMsg::CopyLog).is_empty(), "an empty log copies nothing");
    for _ in 0..3 {
        push_run(&mut st);
    }
    let toasts = st.toasts.len();
    let fx = logs(&mut st, LogsMsg::CopyLog);
    let want = activity_text(&st.log, Lang::En, 0);
    assert_eq!(copied(&fx), [want.as_str()]);
    assert_eq!(st.toasts.len(), toasts + 1);
    let t = st.toasts.last().unwrap();
    assert!(t.ok);
    assert_eq!(t.text, "Copied the activity log to the clipboard (3 entries)");

    st.logs.tab = LogKind::Errors;
    let tail = LogTail { lines: vec!["first".into(), "second".into()], truncated: true };
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Ok(tail)) });
    let fx = logs(&mut st, LogsMsg::CopyLog);
    assert_eq!(copied(&fx), ["(Earlier lines of this file are left out. Export logs has all of it.)\nfirst\nsecond"]);
    assert_eq!(st.toasts.last().unwrap().text, "Copied 2 lines of the error log to the clipboard");

    st.logs.tab = LogKind::Performance;
    let tail = LogTail { lines: vec!["slow".into()], truncated: false };
    st.logs.file = Some(LogFileView { kind: LogKind::Performance, tail: Some(Ok(tail)) });
    let fx = logs(&mut st, LogsMsg::CopyLog);
    assert_eq!(copied(&fx), ["slow"]);
    assert_eq!(st.toasts.last().unwrap().text, "Copied 1 line of the performance log to the clipboard");

    st.logs.file = Some(LogFileView { kind: LogKind::Performance, tail: None });
    let toasts = st.toasts.len();
    assert!(logs(&mut st, LogsMsg::CopyLog).is_empty(), "still being read");
    assert_eq!(st.toasts.len(), toasts);
}
