//! Rules of Preferences > External Tools: planning an editor or terminal launch from the
//! settings, the texts of its failures, and the page's warnings.

use crate::tools::{LaunchFailure, ToolKind};
use std::path::Path;
use trellis_core::external_tools::{self, Launch, TemplateError, Vars};
use trellis_core::settings::ExternalTools;

/// The file rows' menu item.
pub const OPEN_IN_EDITOR: &str = "Open in external editor";
pub const NO_EDITOR: &str = "No external editor is set. Choose one in Preferences > External Tools.";
/// The palette's detail when no editor is set.
pub const NO_EDITOR_DETAIL: &str = "no editor set";

/// The failure text for a template whose field is called `field` on the page.
fn unmatched(field: &str) -> String {
    format!("The {field} arguments in Preferences > External Tools have an unmatched quote.")
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
    };
    match failure {
        LaunchFailure::NotFound => {
            format!("Could not find the external {what} \"{program}\". Check the {what} executable in Preferences > External Tools.")
        }
        LaunchFailure::Missing(p) => format!("{} is not in the working tree", p.display()),
        LaunchFailure::Other(e) => match kind {
            ToolKind::Editor => format!("Could not open {target} in {program}: {e}"),
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

/// The warnings under an arguments field: an unmatched quote, and names that are not
/// placeholders.
pub fn template_warnings(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    if external_tools::split(template).is_err() {
        out.push("These arguments have an unmatched quote; they can't be used until it is fixed.".to_owned());
    }
    out.extend(
        external_tools::unknown_placeholders(template).into_iter().map(|p| format!("{p} is not a placeholder; it is passed as typed.")),
    );
    out
}

/// The palette's detail for Open repository in external editor: the editor's file name, or
/// that none is set.
pub fn editor_detail(t: &ExternalTools) -> String {
    external_tools::program(&t.editor)
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| NO_EDITOR_DETAIL.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_name_the_problem() {
        assert!(template_warnings("{path}").is_empty());
        assert!(template_warnings("").is_empty());
        let w = template_warnings(r#"--goto "{path}"#);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("unmatched quote"), "{w:?}");
        assert_eq!(template_warnings("{path}:{col}"), ["{col} is not a placeholder; it is passed as typed."]);
    }

    #[test]
    fn bundles_are_recognised() {
        assert!(bundle_warning("/Applications/Ed.app"));
        assert!(bundle_warning(" \"/Applications/Ed.APP/\" "));
        assert!(!bundle_warning("/usr/local/bin/ed"));
        assert!(!bundle_warning(""));
    }

    #[test]
    fn editor_detail_is_the_program_name() {
        let t = ExternalTools { editor: "code".into(), ..ExternalTools::default() };
        assert_eq!(editor_detail(&t), "code");
        let t = ExternalTools { editor: "/opt/ed/bin/ed.sh".into(), ..ExternalTools::default() };
        assert_eq!(editor_detail(&t), "ed");
        assert_eq!(editor_detail(&ExternalTools::default()), NO_EDITOR_DETAIL);
    }

    #[test]
    fn terminal_is_none_until_set() {
        let repo = Path::new("/r");
        assert!(terminal(&ExternalTools::default(), repo).is_none());
        let t = ExternalTools { terminal: "wt".into(), terminal_args: "-d \"{repo}".into(), ..ExternalTools::default() };
        assert_eq!(terminal(&t, repo), Some(Err(unmatched("Terminal"))));
    }
}
