//! The palette's external tool entries: open the repository in the external editor, the open
//! diff in the diff tool, the open conflict in the merge tool, Preferences > External tools
//! (the editor, diff and merge tools) and Preferences > Terminal.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::tools::ToolsMsg;

const OPEN_REPO_KEYWORDS: &str = "editor ide code project folder workspace external open";
const CONFIGURE_KEYWORDS: &str = "editor diff merge program tools settings preferences external";
const TERMINAL_KEYWORDS: &str = "terminal shell console external built-in settings preferences";
const DIFF_KEYWORDS: &str = "diff compare tool external program open file";
const MERGE_KEYWORDS: &str = "merge conflict resolve tool external program";

/// Open repository in external editor (with a repository open; the detail names the editor),
/// Open file in external diff tool (while a diff is open) and Resolve conflict in external
/// merge tool (while the conflict view is open), each naming its program, and Configure
/// external tools and Configure terminal (always).
pub(super) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    if st.snap.is_some() {
        let open = entry(PaletteCommand::Tools(ToolsMsg::OpenRepo), "Open repository in external editor", OPEN_REPO_KEYWORDS);
        out.push(with_detail(open, Some(super::tools::program_detail(&st.settings.external_tools.editor, super::tools::NO_EDITOR_DETAIL))));
    }
    let t = &st.settings.external_tools;
    if let Some((target, _)) = &st.diff {
        let open = entry(PaletteCommand::Tools(ToolsMsg::OpenDiff(target.clone())), "Open file in external diff tool", DIFF_KEYWORDS);
        out.push(with_detail(open, Some(super::tools::program_detail(&t.diff_tool, super::tool_compare::NO_DIFF_TOOL_DETAIL))));
    }
    if let Some(c) = &st.session.conflict.view {
        let msg = ToolsMsg::ResolveInMergeTool(c.path.clone());
        let resolve = entry(PaletteCommand::Tools(msg), "Resolve conflict in external merge tool", MERGE_KEYWORDS);
        out.push(with_detail(resolve, Some(super::tools::program_detail(&t.merge_tool, super::tool_compare::NO_MERGE_TOOL_DETAIL))));
    }
    out.push(entry(PaletteCommand::Tools(ToolsMsg::ShowPage), "Configure external tools", CONFIGURE_KEYWORDS));
    out.push(entry(PaletteCommand::ConfigureTerminal, "Configure terminal", TERMINAL_KEYWORDS));
}
