//! The command palette: opening and closing it, moving the selection, and running a command
//! through the message that the rest of the app already uses for it.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::palette::{self, Palette, PaletteCommand};
use crate::state::{AppState, PrefPage, Selection};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::TogglePalette => {
            if st.palette.is_some() {
                st.palette = None;
            } else if st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing() {
                st.palette = Some(Palette::default());
            }
        }
        Msg::ClosePalette => st.palette = None,
        Msg::PaletteMove(step) => {
            let Some(query) = st.palette.as_ref().map(|p| p.query.clone()) else { return };
            let len = palette::shown(st, &query).len();
            let Some(p) = st.palette.as_mut() else { return };
            p.selected = if len == 0 { 0 } else { (p.selected.min(len - 1) as isize + step).rem_euclid(len as isize) as usize };
            p.scroll_to_selected = true;
        }
        Msg::RunPaletteCommand(cmd) => {
            st.palette = None;
            run(st, cmd, now, fx);
        }
        _ => {}
    }
}

/// Runs `cmd` through the message the toolbar, menus and sidebar already send for it.
fn run(st: &mut AppState, cmd: PaletteCommand, now: Instant, fx: &mut Vec<Effect>) {
    use PaletteCommand as C;
    let msg = match cmd {
        C::SearchCommits => {
            if st.diff.is_some() {
                fx.extend(super::update(st, Msg::CloseDiff, now));
            }
            st.view.show_search = true;
            st.view.focus_search = true;
            return;
        }
        C::Fetch => Msg::git("Fetch all", &["fetch", "--all", "--prune"]),
        C::Pull => Msg::git("Pull", &["pull", "--ff"]),
        C::Push => Msg::Push { force: false },
        C::StageAll => Msg::StageAll,
        C::UnstageAll => Msg::UnstageAll,
        C::CreateBranch => {
            let start = match &st.selection {
                Selection::Commit(o) => Some(o.to_string()),
                _ => None,
            };
            Msg::NewBranch { start, name: String::new() }
        }
        C::OpenRepo => Msg::PickRepo,
        C::CloseTab => Msg::CloseTab(usize::MAX),
        C::OpenPrefs => Msg::OpenPrefs(PrefPage::General),
        C::Theme(t) => Msg::SetTheme(t),
        C::Checkout(name) => Msg::git("Checkout", &["checkout", &name]),
        C::OpenRecent(path) => Msg::Open(path),
    };
    fx.extend(super::update(st, msg, now));
}
