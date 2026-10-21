//! Opening a diff in the external diff tool and running the merge tool on a conflict. The diff
//! tool is started and left running (its copies stay for the 24-hour sweep, since many tools
//! hand them to a window that is already open); the merge tool is waited for, its copies are
//! removed when it exits, and how it ended is reported as `MergeToolExited`.

use crate::msg::Msg;
use crate::tools::{DiffToolRequest, LaunchFailure, MergeEnd, MergeToolRequest, ToolKind, ToolsMsg};
use trellis_core::error::Error;
use trellis_core::external_tools::compare::{diff_files, fingerprint, merge_files};
use trellis_core::external_tools::{self, DiffVars, Launch, MergeVars, temp};
use trellis_core::repo;

/// What to say about a failure to prepare the files: the bare reason for the ones Trellis
/// words itself ("no changes in a.txt", "a.txt is a submodule; …").
pub(super) fn reason(e: &Error) -> String {
    match e {
        Error::NotFound(m) | Error::Unsupported(m) | Error::Other(m) => m.clone(),
        e => e.to_string(),
    }
}

pub(super) fn launch_failure(e: &Error) -> LaunchFailure {
    if e.is_not_found() { LaunchFailure::NotFound } else { LaunchFailure::Other(e.to_string()) }
}

/// Writes the diff's versions and starts the diff tool on them. Only a failure is reported.
pub(super) fn open_diff(req: DiffToolRequest) -> Option<Msg> {
    let failed = |failure| {
        let (program, target) = (req.program.display().to_string(), req.target.path().to_owned());
        Some(Msg::Tools(ToolsMsg::LaunchFailed { kind: ToolKind::DiffTool, program, target, failure }))
    };
    let files = match temp::root().map_err(Error::from).and_then(|root| diff_files(&root, &req.workdir, &req.target, req.filters)) {
        Ok(f) => f,
        Err(e) => return failed(LaunchFailure::Files(reason(&e))),
    };
    let path = req.workdir.join(req.target.path());
    let vars = DiffVars { local: &files.local, remote: &files.remote, path: &path, repo: &req.workdir };
    let started = vars.expand(&req.template).map_err(|e| Error::Other(e.to_string())).and_then(|args| {
        external_tools::spawn(&Launch { program: req.program.clone(), args, cwd: req.workdir.clone(), own_console: false })
    });
    match started {
        Ok(()) => None,
        Err(e) => {
            let _ = files.session.remove();
            failed(launch_failure(&e))
        }
    }
}

/// Writes the conflict's stages, runs the merge tool until it exits, removes the copies and
/// says how it ended.
pub(super) fn run_merge(req: MergeToolRequest) -> Msg {
    let end = merge_end(&req);
    let program = req.program.display().to_string();
    Msg::Tools(ToolsMsg::MergeToolExited { workdir: req.workdir, path: req.path, program, end })
}

fn merge_end(req: &MergeToolRequest) -> MergeEnd {
    let files = match temp::root().map_err(Error::from).and_then(|root| merge_files(&root, &req.workdir, &req.path, req.filters)) {
        Ok(f) => f,
        Err(e) => return MergeEnd::Failed(LaunchFailure::Files(reason(&e))),
    };
    let before = fingerprint(&files.output);
    let vars = MergeVars { base: &files.base, ours: &files.ours, theirs: &files.theirs, output: &files.output, repo: &req.workdir };
    let ran = vars.expand(&req.template).map_err(|e| Error::Other(e.to_string())).and_then(|args| {
        external_tools::run_to_exit(&Launch { program: req.program.clone(), args, cwd: req.workdir.clone(), own_console: false })
    });
    let changed = fingerprint(&files.output) != before;
    let _ = files.session.remove();
    match ran {
        Err(e) => MergeEnd::Failed(launch_failure(&e)),
        Ok(r) if !r.success() => MergeEnd::Exited { code: r.code, said: r.said, changed },
        Ok(_) => match repo::is_conflicted(&req.workdir, &req.path) {
            Ok(false) => MergeEnd::Resolved,
            _ if changed => MergeEnd::Saved,
            _ => MergeEnd::Unchanged,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{conflicted, git};
    use super::*;
    use std::path::Path;
    use trellis_core::repo::DiffTarget;

    fn merge(dir: &Path, program: &str, template: &str) -> MergeEnd {
        let req = MergeToolRequest {
            program: program.into(),
            template: template.into(),
            workdir: dir.into(),
            path: "a.txt".into(),
            filters: true,
        };
        match run_merge(req) {
            Msg::Tools(ToolsMsg::MergeToolExited { path, end, .. }) if path == "a.txt" => end,
            _ => panic!("expected MergeToolExited"),
        }
    }

    #[test]
    fn merge_tool_runs_end_as_their_exit_and_the_file_say() {
        let dir = conflicted("merge-tool-run");
        assert_eq!(merge(&dir, "git", "--version"), MergeEnd::Unchanged);
        let e = merge(&dir, "git", "no-such-command {output}");
        assert!(matches!(&e, MergeEnd::Exited { code: Some(c), changed: false, .. } if *c != 0), "{e:?}");
        let missing = format!("trellis-no-such-merge-tool-{}", std::process::id());
        assert_eq!(merge(&dir, &missing, "{output}"), MergeEnd::Failed(LaunchFailure::NotFound));
        // Writes theirs into the working file without resolving the index entry.
        assert_eq!(merge(&dir, "git", "checkout --theirs -- {output}"), MergeEnd::Saved);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "theirs\n");
        // A tool that stages the file resolved it.
        assert_eq!(merge(&dir, "git", "add -- {output}"), MergeEnd::Resolved);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diff_tool_reports_only_failures() {
        let dir = conflicted("diff-tool-open");
        git(&dir, &["checkout", "-q", "--theirs", "--", "a.txt"]);
        git(&dir, &["add", "a.txt"]);
        std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
        let req = |program: &str| DiffToolRequest {
            program: program.into(),
            template: "--version".into(),
            workdir: dir.clone(),
            target: DiffTarget::Unstaged("a.txt".into()),
            filters: false,
        };
        assert!(open_diff(req("git")).is_none());
        let failure = |m: Option<Msg>| match m {
            Some(Msg::Tools(ToolsMsg::LaunchFailed { kind: ToolKind::DiffTool, target, failure, .. })) => (target, failure),
            _ => panic!("expected a diff tool failure"),
        };
        let missing = format!("trellis-no-such-diff-tool-{}", std::process::id());
        assert_eq!(failure(open_diff(req(&missing))), ("a.txt".to_owned(), LaunchFailure::NotFound));
        let nothing = DiffToolRequest { target: DiffTarget::Staged("missing.txt".into()), ..req("git") };
        assert_eq!(failure(open_diff(nothing)), ("missing.txt".to_owned(), LaunchFailure::Files("no changes in missing.txt".into())));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
