//! Rules of Preferences > External tools: planning an editor or terminal launch from the
//! settings, the texts of its failures (the diff and merge tools' too), and the page's
//! warnings. The diff and merge tools' own rules are in state/tool_compare.rs.

use crate::tools::{LaunchFailure, ToolKind};
use glowfly_core::external_tools::{self, Launch, TemplateError, Vars};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::ExternalTools;
use std::path::Path;

/// The file rows' menu item, in English for the log line that names it (update/files.rs).
pub const OPEN_IN_EDITOR: &str = "Open in external editor";
const NO_EDITOR: &str = "No external editor is set. Choose one in Preferences > External tools.";

/// The file rows' menu item.
pub fn open_in_editor(t: Lang) -> &'static str {
    t.tr("tools.open_in_editor")
}

/// The editor entries' hover note while no editor is set (they open the page).
pub fn editor_setup(t: Lang) -> &'static str {
    t.tr("tools.editor_setup.tip")
}
/// The palette's detail when no editor is set.
pub const NO_EDITOR_DETAIL: &str = "no editor set";

/// The failure text for a template whose field is called `field` on the page.
pub(super) fn unmatched(field: &str) -> String {
    format!("The {field} arguments in Preferences > External tools have an unmatched quote.")
}

fn expand(template: &str, vars: &Vars, field: &str) -> Result<Vec<std::ffi::OsString>, String> {
    external_tools::expand(template, vars).map_err(|TemplateError::UnmatchedQuote| unmatched(field))
}

fn editor_launch(t: &ExternalTools, repo: &Path, template: &str, vars: &Vars, field: &str) -> Result<Launch, String> {
    let program = external_tools::program(&t.editor).ok_or_else(|| NO_EDITOR.to_owned())?;
    let args = expand(template, vars, field)?;
    Ok(Launch { program, args, cwd: repo.to_owned(), own_console: false })
}

/// Opening `file` (absolute, in `repo`) at `line` in the editor; the failure text when no
/// editor is set or the template is unusable.
pub fn editor_file(t: &ExternalTools, repo: &Path, file: &Path, line: u32) -> Result<Launch, String> {
    editor_launch(t, repo, &t.editor_file_args, &Vars { path: file, line, repo }, "Open file")
}

/// Opening the repository folder in the editor.
pub fn editor_repo(t: &ExternalTools, repo: &Path) -> Result<Launch, String> {
    editor_launch(t, repo, &t.editor_repo_args, &Vars { path: repo, line: 1, repo }, "Open repository")
}

/// Starting the custom terminal in `repo`; None when none is set (the usual terminal is
/// detected instead).
pub fn terminal(t: &ExternalTools, repo: &Path) -> Option<Result<Launch, String>> {
    let program = external_tools::program(&t.terminal)?;
    Some(expand(&t.terminal_args, &Vars { path: repo, line: 1, repo }, "Terminal").map(|args| Launch {
        program,
        args,
        cwd: repo.to_owned(),
        own_console: true,
    }))
}

/// The log line and toast for a launch that failed.
pub fn failure_text(kind: ToolKind, program: &str, target: &str, failure: &LaunchFailure) -> String {
    let what = match kind {
        ToolKind::Editor => "editor",
        ToolKind::Terminal => "terminal",
        ToolKind::DiffTool => "diff tool",
        ToolKind::MergeTool => "merge tool",
    };
    match failure {
        LaunchFailure::NotFound => {
            format!("Could not find the external {what} \"{program}\". Check the {what} executable in Preferences > External tools.")
        }
        LaunchFailure::Missing(p) => format!("{} is not in the working tree", glowfly_core::native_path::shown(p)),
        LaunchFailure::Files(e) => format!("Could not prepare {target} for the {what}: {e}"),
        LaunchFailure::Other(e) => match kind {
            ToolKind::Editor | ToolKind::DiffTool | ToolKind::MergeTool => format!("Could not open {target} in {program}: {e}"),
            ToolKind::Terminal => format!("Could not start the terminal {program}: {e}"),
        },
    }
}

/// Whether `program` names a macOS application bundle, which may not take every option.
pub fn bundle_warning(program: &str) -> bool {
    external_tools::program(program)
        .and_then(|p| p.to_str().map(|s| s.trim_end_matches('/').to_ascii_lowercase()))
        .is_some_and(|s| s.ends_with(".app"))
}

/// The warnings under an arguments field: an unmatched quote, and names that are not among
/// `known`, that field's placeholders.
pub fn template_warnings(t: Lang, template: &str, known: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    if external_tools::split(template).is_err() {
        out.push(t.tr("prefs.tools.warn.unmatched_quote").to_owned());
    }
    out.extend(
        external_tools::unknown_placeholders(template, known)
            .into_iter()
            .map(|p| t.trf("prefs.tools.warn.not_placeholder", &[("placeholder", &p)])),
    );
    out
}

/// The warnings for each of `wanted` that no argument of `template` uses, so the tool won't be
/// given that version.
pub fn missing_warnings(t: Lang, template: &str, wanted: &[&str]) -> Vec<String> {
    external_tools::missing_placeholders(template, wanted)
        .into_iter()
        .map(|p| t.trf("prefs.tools.warn.missing", &[("placeholder", &p)]))
        .collect()
}

/// The palette's detail for a tool's entry: the file name of `program` (a Preferences
/// field), or `none` when it is not set.
pub fn program_detail(program: &str, none: &str) -> String {
    external_tools::program(program)
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| none.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_name_the_problem() {
        let known = &external_tools::EDITOR_PLACEHOLDERS;
        assert!(template_warnings(Lang::En, "{path}", known).is_empty());
        assert!(template_warnings(Lang::En, "", known).is_empty());
        let w = template_warnings(Lang::En, r#"--goto "{path}"#, known);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("unmatched quote"), "{w:?}");
        assert_eq!(template_warnings(Lang::En, "{path}:{col}", known), ["{col} is not a placeholder; it is passed as typed."]);
        let diff = &external_tools::DIFF_PLACEHOLDERS;
        assert_eq!(template_warnings(Lang::En, "{local} {remote}:{line}", diff), ["{line} is not a placeholder; it is passed as typed."]);
        assert!(template_warnings(Lang::En, "{base} {ours} {theirs} {output} {repo}", &external_tools::MERGE_PLACEHOLDERS).is_empty());
    }

    #[test]
    fn missing_warnings_name_each_absent_placeholder() {
        let want = ["{remote} is missing, so the tool won't be given that version."];
        assert_eq!(missing_warnings(Lang::En, "{local}", &["{local}", "{remote}"]), want);
        assert!(missing_warnings(Lang::En, "{local} --new={remote}", &["{local}", "{remote}"]).is_empty());
        assert_eq!(missing_warnings(Lang::En, "{base} {ours} {theirs}", &["{ours}", "{theirs}", "{output}"]).len(), 1);
    }

    #[test]
    fn diff_and_merge_failures_name_the_tool() {
        let t = failure_text(ToolKind::DiffTool, "meld", "a.txt", &LaunchFailure::NotFound);
        assert_eq!(t, "Could not find the external diff tool \"meld\". Check the diff tool executable in Preferences > External tools.");
        let t = failure_text(ToolKind::DiffTool, "meld", "a.txt", &LaunchFailure::Files("disk full".into()));
        assert_eq!(t, "Could not prepare a.txt for the diff tool: disk full");
        let t = failure_text(ToolKind::MergeTool, "kdiff3", "a.txt", &LaunchFailure::NotFound);
        assert!(t.contains("external merge tool \"kdiff3\""), "{t}");
    }

    #[test]
    fn bundles_are_recognised() {
        assert!(bundle_warning("/Applications/Ed.app"));
        assert!(bundle_warning(" \"/Applications/Ed.APP/\" "));
        assert!(!bundle_warning("/usr/local/bin/ed"));
        assert!(!bundle_warning(""));
    }

    #[test]
    fn program_detail_is_the_program_name() {
        assert_eq!(program_detail("code", NO_EDITOR_DETAIL), "code");
        assert_eq!(program_detail("/opt/ed/bin/ed.sh", NO_EDITOR_DETAIL), "ed");
        assert_eq!(program_detail("", NO_EDITOR_DETAIL), NO_EDITOR_DETAIL);
        assert_eq!(program_detail(" \"\" ", "none"), "none");
    }

    #[test]
    fn terminal_is_none_until_set() {
        let repo = Path::new("/r");
        assert!(terminal(&ExternalTools::default(), repo).is_none());
        let t = ExternalTools { terminal: "wt".into(), terminal_args: "-d \"{repo}".into(), ..ExternalTools::default() };
        assert_eq!(terminal(&t, repo), Some(Err(unmatched("Terminal"))));
    }
}
