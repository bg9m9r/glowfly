//! Starting the external editor or a custom terminal (`Effect::Tool`), and the dialog that
//! picks a tool's program. Trellis never waits for the editor, terminal or diff tool; only a
//! failure to start one is reported. The diff and merge tools are in `compare`, and git config's
//! (when Preferences sets none) in `git_tool`.

use crate::msg::Msg;
use crate::tools::{LaunchFailure, ToolEffect, ToolKind, ToolsMsg};
use glowfly_core::external_tools;
use glowfly_core::external_tools::git_config;

mod compare;
#[cfg(test)]
mod fixture;
mod git_tool;

/// Runs `effect`: a launch reports only a failure, a dialog reports what was picked.
pub(super) fn run(effect: ToolEffect) -> Option<Msg> {
    match effect {
        ToolEffect::Launch { kind, launch, target, check } => {
            let failed = |failure| {
                let program = launch.program.display().to_string();
                Some(Msg::Tools(ToolsMsg::LaunchFailed { kind, program, target: target.clone(), failure }))
            };
            // The file may have gone since its menu was drawn (a broken symlink still counts).
            if let Some(p) = check
                && p.symlink_metadata().is_err()
            {
                return failed(LaunchFailure::Missing(p));
            }
            match external_tools::spawn(&launch) {
                Ok(()) => None,
                Err(e) if e.is_not_found() => failed(LaunchFailure::NotFound),
                Err(e) => failed(LaunchFailure::Other(e.to_string())),
            }
        }
        ToolEffect::PickProgram(kind) => {
            let title = match kind {
                ToolKind::Editor => "Choose the editor program",
                ToolKind::Terminal => "Choose the terminal program",
                ToolKind::DiffTool => "Choose the diff tool program",
                ToolKind::MergeTool => "Choose the merge tool program",
            };
            let mut dialog = rfd::FileDialog::new().set_title(title);
            if cfg!(windows) {
                dialog = dialog.add_filter("Programs", &["exe", "cmd", "bat", "com"]).add_filter("All files", &["*"]);
            }
            Some(Msg::Tools(ToolsMsg::ProgramPicked { kind, path: dialog.pick_file() }))
        }
        ToolEffect::OpenDiff(req) => compare::open_diff(req),
        ToolEffect::RunMerge(req) => Some(compare::run_merge(req)),
        ToolEffect::ReadGitTools { cwd, has_repo, next } => {
            Some(Msg::Tools(ToolsMsg::GitToolsRead { tools: git_config::read(&cwd, has_repo, &[]).map(Box::new), cwd, next }))
        }
        ToolEffect::OpenGitDiff(req) => git_tool::open_diff(req),
        ToolEffect::RunGitMerge(req) => Some(git_tool::run_merge(req)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::external_tools::Launch;
    use std::path::PathBuf;

    fn failure(m: Option<Msg>) -> LaunchFailure {
        match m {
            Some(Msg::Tools(ToolsMsg::LaunchFailed { failure, .. })) => failure,
            _ => panic!("expected a launch failure"),
        }
    }

    fn launch(program: &str) -> Launch {
        Launch { program: program.into(), args: Vec::new(), cwd: glowfly_core::paths::temp_dir(), own_console: false }
    }

    #[test]
    fn a_missing_file_is_reported_before_anything_starts() {
        let gone = glowfly_core::paths::temp_dir().join(format!("trellis-no-such-file-{}", std::process::id())).join("gone.txt");
        // `git` would start; the missing file stops it first.
        let m =
            run(ToolEffect::Launch { kind: ToolKind::Editor, launch: launch("git"), target: "gone.txt".into(), check: Some(gone.clone()) });
        assert_eq!(failure(m), LaunchFailure::Missing(gone));
    }

    #[test]
    fn a_missing_program_is_not_found() {
        let program = format!("trellis-no-such-editor-{}", std::process::id());
        let m = run(ToolEffect::Launch { kind: ToolKind::Terminal, launch: launch(&program), target: "repo".into(), check: None });
        match m {
            Some(Msg::Tools(ToolsMsg::LaunchFailed { kind: ToolKind::Terminal, program: p, target, failure: LaunchFailure::NotFound })) => {
                assert_eq!((PathBuf::from(p), target.as_str()), (PathBuf::from(program), "repo"));
            }
            Some(Msg::Tools(other)) => panic!("expected NotFound, got {other:?}"),
            _ => panic!("expected a launch failure"),
        }
    }
}
