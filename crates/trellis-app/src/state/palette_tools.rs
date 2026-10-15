//! The palette's external tool entries: open the repository in the external editor, and
//! Preferences > External Tools.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::tools::ToolsMsg;

const OPEN_REPO_KEYWORDS: &str = "editor ide code project folder workspace external open";
const CONFIGURE_KEYWORDS: &str = "editor terminal program tools settings preferences external";

/// Open repository in external editor (with a repository open; the detail names the editor),
/// and Configure external tools (always).
pub(super) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    if st.snap.is_some() {
        let open = entry(PaletteCommand::Tools(ToolsMsg::OpenRepo), "Open repository in external editor", OPEN_REPO_KEYWORDS);
        out.push(with_detail(open, Some(super::tools::editor_detail(&st.settings.external_tools))));
    }
    out.push(entry(PaletteCommand::Tools(ToolsMsg::ShowPage), "Configure external tools", CONFIGURE_KEYWORDS));
}
