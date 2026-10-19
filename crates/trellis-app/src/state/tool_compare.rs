//! Rules of the external diff and merge tools: which programs are not set yet (`ToolSetup`: the
//! entries stay enabled and open Preferences > External tools), when the merge tool entry is
//! disabled and why, planning a request from the settings, and what a merge tool run's end says.

use super::AppState;
use super::tools::{EDITOR_SETUP, failure_text, unmatched};
use crate::tools::{DiffToolRequest, LaunchFailure, MergeEnd, MergeToolRequest, ToolKind};
use std::path::Path;
use trellis_core::external_tools;
use trellis_core::repo::{ConflictContent, DiffTarget};
use trellis_core::settings::ExternalTools;

/// The file rows' and diff header's entry.
pub const OPEN_IN_DIFF_TOOL: &str = "Open in external diff tool";
/// The conflicted rows' entry.
pub const RESOLVE_IN_MERGE_TOOL: &str = "Resolve in external merge tool";
/// The conflict view's button.
pub const MERGE_TOOL_BUTTON: &str = "Open in merge tool";
pub const NO_DIFF_TOOL: &str = "No external diff tool is set. Choose one in Preferences > External tools.";
pub const NO_MERGE_TOOL: &str = "No external merge tool is set. Choose one in Preferences > External tools.";
/// The hover notes of the tool entries while no program is set (they open the page).
pub const DIFF_TOOL_SETUP: &str = "No external diff tool is set yet: this opens Preferences > External tools to choose one.";
pub const MERGE_TOOL_SETUP: &str = "No external merge tool is set yet: this opens Preferences > External tools to choose one.";
/// Why the merge tool can't start while a job runs.
pub const BUSY: &str = "Another operation is still running";
const DELETED: &str = "One side deleted the file: keep or delete it with the buttons below";
/// The palette's details when no tool is set.
pub const NO_DIFF_TOOL_DETAIL: &str = "no diff tool set";
pub const NO_MERGE_TOOL_DETAIL: &str = "no merge tool set";

/// Why the conflict view's write actions (Save, Mark resolved, a whole side) are disabled
/// while the merge tool has the file open.
pub const IN_MERGE_TOOL: &str = "Open in the merge tool: save and close it there first";

/// The merge tool job's label for `path`.
pub fn merge_label(path: &str) -> String {
    format!("Resolve {path} in the merge tool")
}

/// Whether the merge tool has `path` of the active tab's repository open: its job is the
/// active one there. Writes to the file from the app would race with the tool's save.
pub fn merging(st: &AppState, path: &str) -> bool {
    let (Some(job), Some(here)) = (&st.job, st.workdir()) else { return false };
    job.label == merge_label(path) && job.workdir.as_deref().is_some_and(|w| trellis_core::paths::same_spelling(w, &here))
}

/// Which external programs are not set yet, each with its entries' hover note. Their entries
/// stay enabled: running one opens Preferences > External Tools (decided in update).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolSetup {
    pub editor: Option<&'static str>,
    pub diff: Option<&'static str>,
    pub merge: Option<&'static str>,
}

impl ToolSetup {
    pub fn of(t: &ExternalTools) -> Self {
        let unset = |program: &str, note| external_tools::program(program).is_none().then_some(note);
        Self {
            editor: unset(&t.editor, EDITOR_SETUP),
            diff: unset(&t.diff_tool, DIFF_TOOL_SETUP),
            merge: unset(&t.merge_tool, MERGE_TOOL_SETUP),
        }
    }
}

/// Why the merge tool can't be offered for a conflict whose `content` is known (None when it
/// isn't loaded): a job running (`busy`), one side deleted the file, or content a tool can't
/// merge. Binary files are fine. No tool set is not a blocker (`ToolSetup`).
pub fn merge_blocked(content: Option<&ConflictContent>, busy: bool) -> Option<String> {
    if busy {
        return Some(BUSY.to_owned());
    }
    match content {
        Some(ConflictContent::DeletedBy(_)) => Some(DELETED.to_owned()),
        Some(ConflictContent::Unsupported(reason)) => Some(format!("{} can't be merged in a tool", capitalized(reason))),
        _ => None,
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Opening `target` of the repository `repo` in the diff tool; the failure text when no tool
/// is set or its arguments have an unmatched quote.
pub fn diff_request(t: &ExternalTools, repo: &Path, target: &DiffTarget, filters: bool) -> Result<DiffToolRequest, String> {
    let program = external_tools::program(&t.diff_tool).ok_or_else(|| NO_DIFF_TOOL.to_owned())?;
    external_tools::split(&t.diff_tool_args).map_err(|_| unmatched("Diff tool"))?;
    Ok(DiffToolRequest { program, template: t.diff_tool_args.clone(), workdir: repo.to_owned(), target: target.clone(), filters })
}

/// Resolving the conflicted `path` of `repo` in the merge tool, as for `diff_request`.
pub fn merge_request(t: &ExternalTools, repo: &Path, path: &str, filters: bool) -> Result<MergeToolRequest, String> {
    let program = external_tools::program(&t.merge_tool).ok_or_else(|| NO_MERGE_TOOL.to_owned())?;
    external_tools::split(&t.merge_tool_args).map_err(|_| unmatched("Merge tool"))?;
    Ok(MergeToolRequest { program, template: t.merge_tool_args.clone(), workdir: repo.to_owned(), path: path.to_owned(), filters })
}

/// How a merge tool run of `program` on `path` ended: whether that is a success, the job's log
/// text and its headline (the toast).
pub fn merge_end_text(program: &str, path: &str, end: &MergeEnd) -> (bool, String, String) {
    let (ok, headline) = match end {
        MergeEnd::Saved => (true, format!("The merge tool saved {path}. Mark it resolved to stage the result.")),
        MergeEnd::Unchanged => (false, format!("The merge tool closed without changing {path}; it is still conflicted.")),
        MergeEnd::Resolved => (true, format!("{path} is resolved.")),
        MergeEnd::Exited { code, said, changed } => {
            let how = match code {
                Some(c) => format!("The merge tool exited with code {c}"),
                None => "The merge tool was stopped".to_owned(),
            };
            let said = if said.is_empty() { String::new() } else { format!(": {said}") };
            let changed = if *changed { " (the file was changed; check it before marking it resolved)" } else { "" };
            (false, format!("{how}{said}; {path} is still conflicted{changed}."))
        }
        MergeEnd::Failed(f) => (false, failure_text(ToolKind::MergeTool, program, path, f)),
    };
    (ok, format!("{program}: {headline}"), headline)
}

/// Whether a merge tool failure is that the program wasn't found (its toast opens Preferences).
pub fn not_found(end: &MergeEnd) -> bool {
    matches!(end, MergeEnd::Failed(LaunchFailure::NotFound))
}

/// What file rows need for their tool entries: which programs are not set yet, and why the
/// merge tool item is disabled (the content is not known on rows, so only busy counts).
pub struct ToolAvail {
    pub setup: ToolSetup,
    pub merge: Option<String>,
}

impl ToolAvail {
    pub fn of(st: &AppState) -> Self {
        Self { setup: ToolSetup::of(&st.settings.external_tools), merge: merge_blocked(None, st.job.is_some()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::repo::Side;

    fn tools(diff: &str, merge: &str) -> ExternalTools {
        ExternalTools { diff_tool: diff.into(), merge_tool: merge.into(), ..ExternalTools::default() }
    }

    #[test]
    fn entries_say_why_they_are_disabled() {
        assert_eq!(merge_blocked(None, false), None, "no merge tool set is not a blocker");
        assert_eq!(merge_blocked(None, true).as_deref(), Some(BUSY));
        assert_eq!(merge_blocked(Some(&ConflictContent::DeletedBy(Side::Theirs)), false).as_deref(), Some(DELETED));
        let sub = ConflictContent::Unsupported("a submodule".into());
        assert_eq!(merge_blocked(Some(&sub), false).as_deref(), Some("A submodule can't be merged in a tool"));
        assert_eq!(merge_blocked(Some(&ConflictContent::Binary), false), None, "binary files are fine");
        assert_eq!(merge_blocked(Some(&ConflictContent::Text(Vec::new())), false), None);
    }

    #[test]
    fn tool_setup_names_what_is_unset() {
        let unset = ToolSetup { editor: Some(EDITOR_SETUP), diff: Some(DIFF_TOOL_SETUP), merge: Some(MERGE_TOOL_SETUP) };
        assert_eq!(ToolSetup::of(&tools("", "")), unset);
        let quoted = ExternalTools { editor: " \"\" ".into(), ..tools(" \"\" ", " \"\" ") };
        assert_eq!(ToolSetup::of(&quoted), unset, "an empty quoted program counts as unset");
        let set = ExternalTools { editor: "code".into(), ..tools("meld", "kdiff3") };
        assert_eq!(ToolSetup::of(&set), ToolSetup::default());
        assert_eq!(ToolSetup::of(&tools("meld", "")).merge, Some(MERGE_TOOL_SETUP));
    }

    #[test]
    fn requests_need_a_program_and_usable_arguments() {
        let repo = Path::new("/r");
        let target = DiffTarget::Unstaged("a.txt".into());
        assert_eq!(diff_request(&tools("", ""), repo, &target, true), Err(NO_DIFF_TOOL.to_owned()));
        let mut t = tools("meld", "kdiff3");
        let r = diff_request(&t, repo, &target, false).unwrap();
        assert_eq!(
            r,
            DiffToolRequest { program: "meld".into(), template: "{local} {remote}".into(), workdir: repo.into(), target, filters: false }
        );
        t.diff_tool_args = "\"{local} {remote}".into();
        let e = diff_request(&t, repo, &DiffTarget::Staged("a.txt".into()), true).unwrap_err();
        assert_eq!(e, "The Diff tool arguments in Preferences > External tools have an unmatched quote.");
        assert_eq!(merge_request(&tools("meld", ""), repo, "a.txt", true), Err(NO_MERGE_TOOL.to_owned()));
        let r = merge_request(&t, repo, "a.txt", true).unwrap();
        assert_eq!(
            (r.program.to_str(), r.template.as_str(), r.path.as_str(), r.filters),
            (Some("kdiff3"), "{base} {ours} {theirs} {output}", "a.txt", true)
        );
        t.merge_tool_args = "{output} \"".into();
        assert!(merge_request(&t, repo, "a.txt", true).unwrap_err().contains("Merge tool arguments"));
    }

    #[test]
    fn merge_ends_read_as_success_or_failure() {
        let text = |e: MergeEnd| merge_end_text("kdiff3", "a.txt", &e);
        let (ok, out, head) = text(MergeEnd::Saved);
        assert!(ok && head == "The merge tool saved a.txt. Mark it resolved to stage the result." && out.contains(&head));
        let (ok, _, head) = text(MergeEnd::Unchanged);
        assert!(!ok && head == "The merge tool closed without changing a.txt; it is still conflicted.");
        assert_eq!(text(MergeEnd::Resolved), (true, "kdiff3: a.txt is resolved.".into(), "a.txt is resolved.".into()));
        let (ok, _, head) = text(MergeEnd::Exited { code: Some(2), said: "bad option".into(), changed: false });
        assert!(!ok);
        assert_eq!(head, "The merge tool exited with code 2: bad option; a.txt is still conflicted.");
        let (_, _, head) = text(MergeEnd::Exited { code: Some(1), said: String::new(), changed: true });
        assert_eq!(
            head,
            "The merge tool exited with code 1; a.txt is still conflicted (the file was changed; check it before marking it resolved)."
        );
        let (ok, _, head) = text(MergeEnd::Failed(LaunchFailure::NotFound));
        assert!(!ok && head.contains("Could not find the external merge tool \"kdiff3\""), "{head}");
        let (_, _, head) = text(MergeEnd::Failed(LaunchFailure::Files("disk full".into())));
        assert_eq!(head, "Could not prepare a.txt for the merge tool: disk full");
        assert!(not_found(&MergeEnd::Failed(LaunchFailure::NotFound)) && !not_found(&MergeEnd::Saved));
    }
}
