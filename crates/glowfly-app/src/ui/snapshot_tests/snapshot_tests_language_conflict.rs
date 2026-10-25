//! German for the conflict view (panes, Take all, chunk titles, the automatic tag, the Output
//! strip), a binary file's fallback and its confirmation, the conflicted and resolved file
//! lists, the conflict check's popover and report, and the in-progress banners: no English text
//! from these areas, checked as `snapshot_tests_language_main` does.

use super::snapshot_tests::{harness, remove_fixture, with_conflict};
use super::snapshot_tests_conflict_check::{conflicts, on_search, report};
use super::snapshot_tests_conflict_files::{fill_lists, merging};
use super::snapshot_tests_language_check::{english_shown, shown_texts};
use super::snapshot_tests_language_main::{H, german};
use super::snapshot_tests_patch::applying;
use super::snapshot_tests_rebase_edit::rebasing;
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::conflict_check::{Prediction, TargetKind};
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::StopKind;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// What these areas showed in English. A `*` stands for a name or number.
const ENGLISH: &[&str] = &[
    // The conflict view.
    "Loading the conflict…",
    "Next conflict",
    "Previous conflict",
    "Binary file: pick a whole version.",
    "Use ours (*)",
    "Use theirs (*)",
    "Delete file (*)",
    "Resolve conflict",
    "Replace",
    "Ours",
    "Theirs",
    "no common ancestor",
    "Take all",
    "Take",
    "Take this line",
    "edited in the output",
    "picked automatically · *",
    "Discard edit",
    "* unchanged line",
    "* unchanged lines",
    "Show these lines",
    "Output",
    "Edit",
    "Done",
    "Wrap lines",
    "Discard edits outside conflicts",
    "* of * conflicts resolved",
    "Conflict *: pick a side above",
    "Edits saved in the file between the conflicts are back.",
    "* of * conflicts were picked automatically (one side unchanged, or only whitespace differs). Check them before saving.",
    "unchanged on one side",
    "same on both sides",
    "whitespace only",
    // The file lists.
    "Conflicted files (*)",
    "Resolved files (*)",
    "Mark all resolved",
    "Mark all conflicted",
    "Mark conflicted",
    "no markers left",
    "* conflict",
    "* conflicts",
    // The conflict check.
    "Conflicts with *",
    "Check again",
    "Conflict check",
    "Merge * into *",
    "Rebase * onto *",
    "Merging * into * would stop with conflicts.",
    "Merging * into * would stop with conflicts in * files:",
    "* file changed on both sides:",
    "* files changed on both sides:",
    "Turn this check off in Preferences > General.",
    "and * more",
    "* is this repository's default branch.",
    "* is the upstream of *.",
    "Merging * into * would stop with conflicts in * files.",
    "Click to see the files and merge or rebase.",
    "Click for details.",
    // The banners.
    "Merge in progress",
    "Interactive rebase in progress · step * of *",
    "Applying patches · patch * of *",
    "Paused to edit *. Amend it or add commits, then Continue.",
    "Stopped at: *",
    "Resolve the conflicted files, then Continue.",
    "Edit remaining steps…",
    "Your uncommitted changes are stashed until the rebase ends.",
];

fn assert_no_english(h: &H, what: &str) {
    let found = english_shown(h, ENGLISH, 50);
    assert!(found.is_empty(), "{what} still shows English: {found:?}");
}

fn shows_containing(h: &H, text: &str) -> bool {
    shown_texts(h).iter().any(|t| t.contains(text))
}

/// settings.txt with two conflicts: the first only differs in whitespace (picked
/// automatically), the second is a real one. The commit panel is expanded beside the view.
fn auto_picked(name: &str) -> (AppState, PathBuf) {
    let settings = |side: &str| {
        let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
        match side {
            "ours" => {
                lines[2] = "    setting_3 = 3".into();
                lines[8] = "font_size = 13".into();
            }
            "theirs" => {
                lines[2] = "setting_3  =  3".into();
                lines[8] = "font_size = 14".into();
            }
            _ => {}
        }
        (lines.join("\n") + "\n").into_bytes()
    };
    let (mut st, dir) = with_conflict(name, "settings.txt", settings);
    assert_eq!(st.conflict.as_ref().unwrap().auto_count(), 1);
    st.view.details_expanded_for = crate::state::layout::tool_view(&st);
    (st, dir)
}

#[test]
fn conflict_view_german() {
    let de = Lang::De;
    let (mut st, dir) = auto_picked("lang-conflict-view");
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    for key in ["conflict.panes.ours", "conflict.panes.theirs", "conflict.output.title", "conflict.panes.take_all.button"] {
        assert!(shows_containing(&h, de.tr(key)), "{key} in German");
    }
    assert!(shows_containing(&h, &de.trf("conflict.output.progress", &[("done", &1), ("total", &2)])));
    assert_no_english(&h, "the conflict view");
    h.check_snapshot("conflict_view_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_binary_german() {
    let de = Lang::De;
    let (mut st, dir) = with_conflict("lang-conflict-binary", "logo.bin", |side| format!("{side}\0image").into_bytes());
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(de.tr("conflict.view.binary")).is_some(), "the note is German");
    assert_no_english(&h, "a binary conflict");
    h.check_snapshot("conflict_binary_german");
    let ours = de.trf("conflict.view.use_ours.button", &[("name", &"")]);
    h.query_by_label_contains(ours.trim_end_matches("()").trim_end()).expect("Use ours in German").click();
    h.run();
    let asked = h.state().1.iter().rev().find_map(|m| match m {
        Msg::ConfirmDanger { title, body, button, .. } => Some((title.clone(), body.clone(), button.clone())),
        _ => None,
    });
    let body = de.trf("conflict.view.use_ours.body", &[("path", &"logo.bin")]);
    let want = (de.tr("conflict.view.resolve.title").to_owned(), body, de.tr("conflict.view.replace.button").to_owned());
    assert_eq!(asked, Some(want), "the confirmation is German");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_lists_german() {
    let de = Lang::De;
    let (mut st, dir) = merging("lang-conflict-lists", true);
    fill_lists(&mut st, &dir);
    german(&mut st);
    st.settings.panel_sizes.details = Some(520.0);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(de.tr("conflict.files.mark_all_resolved.button")).is_some());
    assert_no_english(&h, "the conflict lists");
    h.check_snapshot("conflict_lists_german");
    h.get_by_label(de.tr("conflict.files.mark_all_conflicted.button")).click();
    h.run();
    let asked = h.state().1.iter().rev().find_map(|m| match m {
        Msg::Confirm { title, body, .. } => Some((title.clone(), body.clone())),
        _ => None,
    });
    let body = crate::state::conflict_files::mark_conflicted_body(de, &["gamma.txt".to_owned()]);
    assert_eq!(asked, Some((de.tr("conflict.files.mark_conflicted.title").to_owned(), body)), "the confirmation is German");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_check_german() {
    let de = Lang::De;
    let files = conflicts(&["src/app.rs", "README.md"]);
    let (mut st, dir, _) = on_search(ThemeChoice::Dark, "lang-conflict-check", files);
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(&de.trf("conflict_check.chip.name", &[("target", &"main")])).click();
    h.run();
    assert!(h.query_by_label(de.tr("conflict_check.check_again.button")).is_some(), "the popover is German");
    assert_no_english(&h, "the conflict popover");
    h.check_snapshot("conflict_popover_german");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = super::snapshot_tests::with_repo(ThemeChoice::Dark, "lang-conflict-report");
    let paths: Vec<String> = (0..14).map(|i| format!("src/module_{i:02}.rs")).collect();
    st.modal = Some(Modal::ConflictReport(report(&st, TargetKind::DefaultBranch, Ok(Prediction::Conflicts(paths)))));
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&de.trf("conflict_check.merge.button", &[("theirs", &"search"), ("ours", &"main")])).is_some());
    assert_no_english(&h, "the conflict report");
    h.check_snapshot("conflict_report_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn banners_german() {
    let de = Lang::De;
    let (mut st, dir) = merging("lang-banner-merge", false);
    fill_lists(&mut st, &dir);
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(shows_containing(&h, de.tr("details.banner.title.merge")), "the merge banner is German");
    assert_no_english(&h, "the merge banner");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = rebasing(ThemeChoice::Dark, "lang-banner-rebase", StopKind::Edit, false);
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    let title = de.trf("details.banner.title.interactive_step", &[("step", &2), ("total", &4)]);
    assert!(shows_containing(&h, &title), "{title}");
    // Step 3 is one split button; Edit remaining steps… is in its menu.
    let more = h.get_by_label(de.tr("details.steps.finish.more.button")).bounding_box().unwrap();
    let cont = h.get_by_label(de.tr("details.banner.action.continue")).bounding_box().unwrap();
    assert!((more.y0 - cont.y0).abs() < 1.0, "Continue and its caret on one row");
    assert_no_english(&h, "the rebase banner");
    h.check_snapshot("rebase_banner_german");
    h.get_by_label(de.tr("details.steps.finish.more.button")).click();
    h.run();
    assert!(h.query_by_label(de.tr("details.banner.edit_steps.button")).is_some());
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = applying(ThemeChoice::Dark, "lang-banner-am", true);
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(shows_containing(&h, de.tr("details.banner.am.hint.conflicts")), "the patch hint is German");
    assert_no_english(&h, "the patch banner");
    h.check_snapshot("am_banner_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_check_catches_english() {
    let (st, dir) = auto_picked("lang-conflict-english");
    let mut h = harness(st, true);
    h.run();
    let found = english_shown(&h, ENGLISH, 50);
    for text in ["Ours", "Theirs", "Output", "Take all"] {
        assert!(found.iter().any(|f| f == text), "{text} is caught: {found:?}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_check_knows_the_chip_tooltip() {
    use super::snapshot_tests_language_check::{checkable, matches};
    use crate::conflict_check::CheckRequest;
    use crate::state::conflict_check::{ConflictReport, chip_tooltip};
    use glowfly_core::conflict_check::MergeTarget;
    let target = MergeTarget { name: "main".into(), oid: git2::Oid::zero(), kind: TargetKind::DefaultBranch };
    let request = CheckRequest { branch: "search".into(), ours: git2::Oid::zero(), target };
    let report = ConflictReport { request, result: conflicts(&["a.txt", "b.txt"]) };
    let entries = checkable(ENGLISH);
    let caught = |line: &str| entries.iter().any(|e| matches(e, line));
    for line in chip_tooltip(Lang::En, &report, false).lines() {
        assert!(caught(line), "{line} is caught");
    }
    for line in chip_tooltip(Lang::De, &report, false).lines() {
        assert!(!caught(line), "{line} is German");
    }
}
