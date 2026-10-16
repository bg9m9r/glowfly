//! Questions that come from what the operating system would do: retrying a move to the trash
//! that another program blocked.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::effect::{DeleteMode, FileOp};
use crate::msg::{Msg, WorkFileMsg};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::repo::{TrashProblem, TrashRefusal};
use trellis_core::settings::ThemeChoice;

#[test]
fn trash_in_use_retry() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "trash-in-use");
    let refusal = TrashRefusal::new(TrashProblem::InUse, "in use");
    let op = FileOp::Delete { path: "notes/scratch.txt".into(), mode: DeleteMode::Trash };
    crate::update::update(&mut st, Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal }), std::time::Instant::now());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("trash_in_use_retry");
    assert!(h.query_by_label("Delete permanently").is_none(), "no permanent delete for a file in use");
    h.get_by_label("Retry").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "Retry accepts");
    drop(h);
    remove_fixture(&dir);
}
