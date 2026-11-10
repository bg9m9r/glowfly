//! Tests of what a save of recent.txt or tabs.txt reports back to the app.

use super::*;
use std::path::PathBuf;

#[test]
fn report_maps_each_outcome() {
    assert!(report(StateFile::Recent, false, "the recent repositories", Ok(SaveNotes::default())).is_none());
    let notes = SaveNotes { read_late: Some(vec![PathBuf::from("/w/a")]), kept: None };
    let msg = report(StateFile::Tabs, false, "the open tabs", Ok(notes.clone()));
    assert!(matches!(&msg, Some(Msg::Recent(RecentMsg::StateFileSaved { file: StateFile::Tabs, notes: n })) if *n == notes));
    let refused = Err(SaveRefused::Unread { detail: "d".into(), report: true });
    let msg = report(StateFile::Recent, true, "the recent repositories", refused);
    assert!(matches!(
        &msg,
        Some(Msg::Recent(RecentMsg::StateFileBlocked { file: StateFile::Recent, clearing: true, detail })) if detail == "d"
    ));
    let quiet = Err(SaveRefused::Unread { detail: "d".into(), report: false });
    assert!(report(StateFile::Recent, false, "the recent repositories", quiet).is_none());
    let io = Err(SaveRefused::Io(std::io::Error::other("disk full")));
    let msg = report(StateFile::Tabs, false, "the open tabs", io);
    assert!(matches!(&msg, Some(Msg::Logged { ok: false, text }) if text.starts_with("Could not save the open tabs")));
}
