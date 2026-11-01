//! Preferences > External tools snapshots: the page with nothing set (the terminal arguments
//! disabled), and with an editor, custom arguments naming an unknown placeholder, a custom
//! terminal, and diff and merge tools whose arguments leave out a version; with git config's
//! tools under the empty fields (a command and a built-in tool, an unknown one); the conflict view
//! with the merge tool blocked by a job, and with the merge tool open on its file.

use super::snapshot_tests::{harness_sized, settings};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use glowfly_core::settings::{ExternalTools, ThemeChoice};
use std::time::Instant;

const MERGE_TOOL_BUTTON: &str = "Open in merge tool";

fn page(theme: ThemeChoice, tools: ExternalTools) -> AppState {
    let mut s = settings(theme);
    s.external_tools = tools;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    // No git configuration is needed, so the page draws without waiting for it.
    st.prefs = Some(PrefsState::new(PrefPage::Tools));
    st
}

/// Renders the page in a window tall enough for all four sections.
fn render_tall(name: &str, st: AppState, dark: bool) {
    let mut h = harness_sized(st, dark, eframe::egui::vec2(1400.0, 1420.0));
    h.run();
    h.check_snapshot(name);
}

#[test]
fn preferences_tools() {
    render_tall("prefs_tools", page(ThemeChoice::Dark, ExternalTools::default()), true);
}

#[test]
fn preferences_tools_custom_light() {
    let tools = ExternalTools {
        editor: "C:/Tools/Editor/editor.exe".into(),
        editor_file_args: "--goto \"{path}:{line}:{col}\"".into(),
        terminal: "wt".into(),
        terminal_args: "-d \"{repo}\"".into(),
        diff_tool: "C:/Tools/Compare/compare.exe".into(),
        diff_tool_args: "--left {local}".into(),
        merge_tool: "merge".into(),
        merge_tool_args: "{base} {ours} {theirs}".into(),
        ..ExternalTools::default()
    };
    render_tall("prefs_tools_custom_light", page(ThemeChoice::Light, tools), false);
}

#[test]
fn preferences_tools_git_config() {
    use glowfly_core::external_tools::git_config::{GitTool, GitToolRun, GitTools};
    let mut st = page(ThemeChoice::Dark, ExternalTools::default());
    let cmd = GitToolRun::Command {
        key: "difftool.meld.cmd".into(),
        cmd: "meld \"$LOCAL\" \"$REMOTE\"".into(),
        trust_exit_code: false,
        from_repo: false,
    };
    let diff = GitTool { name: "meld".into(), chosen_by: "diff.tool".into(), run: cmd };
    let merge = GitTool { name: "vimdiff".into(), chosen_by: "merge.tool".into(), run: GitToolRun::BuiltIn { path: None } };
    st.prefs.as_mut().unwrap().git_tools = Some(GitTools { diff: Some(diff), merge: Some(merge) });
    render_tall("prefs_tools_git_config", st, true);
}

#[test]
fn preferences_tools_git_config_unknown_light() {
    use glowfly_core::external_tools::git_config::{GitTool, GitToolRun, GitTools};
    let mut st = page(ThemeChoice::Light, ExternalTools::default());
    let diff = GitTool { name: "compare-tool".into(), chosen_by: "merge.tool".into(), run: GitToolRun::Unknown };
    st.prefs.as_mut().unwrap().git_tools = Some(GitTools { diff: Some(diff), merge: None });
    render_tall("prefs_tools_git_config_unknown_light", st, false);
}

#[test]
fn diff_header_diff_tool_set_light() {
    use egui_kittest::kittest::Queryable;
    let (mut st, dir) = super::snapshot_tests_diff::with_diff(glowfly_core::settings::DiffLayout::Inline, "diff-tool-set");
    st.settings.theme = ThemeChoice::Light;
    st.settings.external_tools.diff_tool = "meld".into();
    let mut h = super::snapshot_tests::harness(st, false);
    h.run();
    // The header looks the same with a tool set; its button opens the file in it.
    h.get_by_label("Open in external diff tool").click();
    h.run();
    let sent = |m: &crate::msg::Msg| matches!(m, crate::msg::Msg::Tools(crate::tools::ToolsMsg::OpenDiff(t)) if t.path() == "app.txt");
    assert!(h.state().1.iter().any(sent), "Open in external diff tool opens app.txt's diff");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_view_merge_tool_disabled() {
    use egui_kittest::kittest::Queryable;
    let (mut st, dir) = super::snapshot_tests::with_conflict("conflict-tool-busy", "notes.txt", |side| format!("{side}\n").into_bytes());
    st.settings.external_tools.merge_tool = "kdiff3".into();
    st.job = Some(crate::state::RunningJob {
        label: "Commit".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: false,
        fetches: false,
    });
    // The status bar's job spinner keeps repainting; a fixed number of frames keeps the image
    // reproducible.
    let mut h = super::snapshot_tests::harness(st, true);
    h.run_steps(4);
    h.check_snapshot("conflict_view_merge_tool_disabled");
    h.get_by_label("More conflict actions").click();
    h.run_steps(4);
    assert!(h.get_by_label(MERGE_TOOL_BUTTON).is_disabled(), "a job is running");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_view_merge_tool_button_sends_resolve() {
    use egui_kittest::kittest::Queryable;
    let (mut st, dir) = super::snapshot_tests::with_conflict("conflict-tool-click", "notes.txt", |side| format!("{side}\n").into_bytes());
    st.settings.external_tools.merge_tool = "kdiff3".into();
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    h.get_by_label("More conflict actions").click();
    h.run();
    h.get_by_label(MERGE_TOOL_BUTTON).click();
    h.run();
    let sent =
        h.state().1.iter().any(|m| matches!(m, crate::msg::Msg::Tools(crate::tools::ToolsMsg::ResolveInMergeTool(p)) if p == "notes.txt"));
    assert!(sent);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_view_while_the_merge_tool_has_the_file() {
    use egui_kittest::kittest::Queryable;
    let (mut st, dir) = super::snapshot_tests::with_conflict("conflict-in-tool", "notes.txt", |side| format!("{side}\n").into_bytes());
    st.settings.external_tools.merge_tool = "kdiff3".into();
    st.job = Some(crate::state::RunningJob {
        label: crate::state::tool_compare::merge_label("notes.txt"),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    let mut h = super::snapshot_tests::harness(st, true);
    h.run_steps(4);
    h.check_snapshot("conflict_view_in_merge_tool");
    assert!(h.get_by_label("Save and mark resolved").is_disabled());
    // The header menu's (the file row's is refused when it arrives: update/tool_compare_tests.rs).
    h.get_by_label("More conflict actions").click();
    h.run_steps(4);
    let disabled = h.get_all_by_label("Mark resolved").filter(|n| n.is_disabled()).count();
    assert_eq!(disabled, 1, "staging now would stage the markers");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
