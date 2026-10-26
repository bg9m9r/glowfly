//! The pull request panel's code comment threads placed on the local version's lines, when the
//! files were loaded from a source tip that isn't the service's head: the note over the pane,
//! the marker on the mapped line, and the location links of a mapped and an unmapped thread.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_pull_request_threads::{comment, detail, files_loaded, load, thread};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::PrTab;
use crate::state::pull_request_line_map::{LineMaps, wanted};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{LineChange, LineMap, PrLineMaps};
use glowfly_core::settings::ThemeChoice;
use std::sync::Arc;

#[test]
fn pull_request_mapped_threads_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-mapped-threads", true);
    let (target, d, line) = files_loaded(&mut st, &dir);
    let head = format!("c0ffee0{}", "0".repeat(33));
    // The head has two more lines above the local added line: T1 is on it, T2 on a line that
    // isn't here.
    let t1 = thread("T1", line + 2, vec![comment(55, "linus", 13, "Should this be case-insensitive?", false)]);
    let t2 = thread("T2", line, vec![comment(56, "grace", 14, "This one was dropped.", false)]);
    load(&mut st, detail(head), vec![t1, t2]);
    let v = st.session.pull_request.view.as_mut().unwrap();
    let change = LineChange { old_start: line, old_lines: 2, new_start: line - 1, new_lines: 0 };
    let files = [("search.txt".to_owned(), LineMap { path: Some("search.txt".into()), changes: Some(vec![change]) })].into();
    v.lines =
        LineMaps::Loaded { key: wanted(v).expect("the files aren't of the head"), maps: PrLineMaps::Mapped { old_kept: true, files } };
    v.tab = PrTab::Files;
    assert!(v.shows(&target));
    st.diff = Some((target, Arc::new(d)));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_mapped_threads_dark");
    assert!(h.query_by_label_contains("were made on c0ffee0 and are shown on the local version").is_some());
    assert!(h.query_by_label_contains("fetch origin").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "1 comment on this line").is_some());
    assert!(h.get_by_label(&format!("line {line} on c0ffee0, not in the local version")).is_disabled());
    h.get_by_label(&format!("line {line} (line {} on c0ffee0)", line + 2)).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::GoToThread(id)) if id == "T1")));
    drop(h);
    remove_fixture(&dir);
}
