//! The palette's custom command entries: Reload custom commands, then each command shown in the
//! palette (the user's, then the repository's), its detail saying why it can't run here, else
//! which file it is from (and that the repository's ask to trust it first).

use super::AppState;
use super::custom_commands::{missing_text, repo_trusted, source_detail, vars_for};
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::custom_commands::{CommandRef, CustomCommandMsg, Target};
use glowfly_core::custom_commands::{Place, Source};
use glowfly_core::repo::Snapshot;

const RELOAD_KEYWORDS: &str = "custom commands reload toml file refresh";
const KEYWORDS: &str = "custom command script user";

pub(super) fn entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let t = st.settings.lang();
    out.push(entry(PaletteCommand::Custom(CustomCommandMsg::Reload), t.tr("commands.palette.reload"), RELOAD_KEYWORDS));
    let selected = match st.selection {
        super::Selection::Commit(o) => Some(o),
        _ => None,
    };
    let open_file = st.diff.as_ref().map(|(d, _)| d.path());
    let vars = vars_for(snap, &Target::Palette, selected, open_file);
    let trusted = repo_trusted(st);
    for (source, cmd) in st.custom_commands.listed(Some(&snap.workdir)).filter(|(_, c)| c.shows_in(Place::Palette)) {
        let held = source == Source::Repo && !trusted;
        let detail = match cmd.missing(&vars.as_vars()) {
            Some(m) => missing_text(t, m).to_owned(),
            None => source_detail(t, source, held),
        };
        let run = CustomCommandMsg::Run { command: CommandRef { source, name: cmd.name.clone() }, at: Target::Palette, confirmed: false };
        out.push(with_detail(entry(PaletteCommand::Custom(run), cmd.name.clone(), KEYWORDS), Some(detail)));
    }
}
