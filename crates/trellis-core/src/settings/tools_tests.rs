//! External Tools settings: defaults for old files and partial objects, and the argument resets.

use super::*;
use crate::settings::Settings;

#[test]
fn external_tools_old_files_and_partial_objects() {
    // Files written before Preferences > External tools existed load with the defaults.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.external_tools, ExternalTools::default());
    assert_eq!(
        (s.external_tools.editor_file_args.as_str(), s.external_tools.editor_repo_args.as_str()),
        (EDITOR_FILE_ARGS, EDITOR_REPO_ARGS)
    );
    assert_eq!((EDITOR_FILE_ARGS, EDITOR_REPO_ARGS), ("{path}", "{repo}"));
    assert!(s.external_tools.editor.is_empty() && s.external_tools.terminal.is_empty() && s.external_tools.terminal_args.is_empty());
    // Only the editor set: the argument templates keep their defaults.
    let s: Settings = serde_json::from_str(r#"{"external_tools":{"editor":"x"}}"#).unwrap();
    assert_eq!(s.external_tools, ExternalTools { editor: "x".into(), ..ExternalTools::default() });
}

#[test]
fn reset_editor_args_restores_only_the_templates() {
    let mut t = ExternalTools {
        editor: "code".into(),
        editor_file_args: "--goto {path}:{line}".into(),
        editor_repo_args: "-n {repo}".into(),
        terminal: "wt".into(),
        terminal_args: "-d {repo}".into(),
        diff_tool: "meld".into(),
        diff_tool_args: "{remote}".into(),
        merge_tool: "kdiff3".into(),
        merge_tool_args: "{output}".into(),
    };
    t.reset_editor_args();
    let custom = ExternalTools {
        editor: "code".into(),
        terminal: "wt".into(),
        terminal_args: "-d {repo}".into(),
        diff_tool: "meld".into(),
        diff_tool_args: "{remote}".into(),
        merge_tool: "kdiff3".into(),
        merge_tool_args: "{output}".into(),
        ..ExternalTools::default()
    };
    assert_eq!(t, custom);
}

#[test]
fn diff_and_merge_tools_load_with_defaults_and_reset_alone() {
    assert_eq!((DIFF_TOOL_ARGS, MERGE_TOOL_ARGS), ("{local} {remote}", "{base} {ours} {theirs} {output}"));
    for json in [r#"{"auto_fetch_minutes": 5}"#, r#"{"external_tools":{"editor":"x"}}"#] {
        let t = serde_json::from_str::<Settings>(json).unwrap().external_tools;
        assert!(t.diff_tool.is_empty() && t.merge_tool.is_empty(), "{json}");
        assert_eq!((t.diff_tool_args.as_str(), t.merge_tool_args.as_str()), (DIFF_TOOL_ARGS, MERGE_TOOL_ARGS), "{json}");
    }
    let custom = ExternalTools {
        editor: "code".into(),
        editor_file_args: "--goto {path}".into(),
        diff_tool: "meld".into(),
        diff_tool_args: "{remote} {local}".into(),
        merge_tool: "kdiff3".into(),
        merge_tool_args: "{output}".into(),
        ..ExternalTools::default()
    };
    let mut t = custom.clone();
    t.reset_diff_tool_args();
    assert_eq!(t, ExternalTools { diff_tool_args: DIFF_TOOL_ARGS.into(), ..custom.clone() });
    let mut t = custom.clone();
    t.reset_merge_tool_args();
    assert_eq!(t, ExternalTools { merge_tool_args: MERGE_TOOL_ARGS.into(), ..custom });
}
