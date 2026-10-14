//! The command palette: opening and closing it, moving the selection, running a command
//! through the message that the rest of the app already uses for it, and the sub-lists that
//! two-step commands open (loading their items in the background).

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::palette::{self, Items, Mode, Palette, PaletteCommand, Pick, PickKind};
use crate::state::{AppState, PrefPage, Selection};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::TogglePalette => {
            if st.palette.is_some() {
                st.palette = None;
            } else if may_open(st) {
                st.palette = Some(Palette::default());
            }
        }
        Msg::ClosePalette => st.palette = None,
        Msg::PaletteMove(step) => {
            let len = palette::row_count(st);
            let Some(p) = st.palette.as_mut() else { return };
            p.selected = if len == 0 { 0 } else { (p.selected.min(len - 1) as isize + step).rem_euclid(len as isize) as usize };
            p.scroll_to_selected = true;
        }
        Msg::RunPaletteCommand(cmd) => {
            if let Some(kind) = cmd.pick_kind() {
                enter_pick(st, kind, fx);
                return;
            }
            st.palette = None;
            run(st, cmd, now, fx);
        }
        Msg::PaletteBack => {
            if let Some(p) = st.palette.as_mut()
                && matches!(p.mode, Mode::Pick(_))
            {
                p.mode = Mode::Commands;
                p.query.clear();
                p.selected = 0;
            }
        }
        Msg::TrackedPathsLoaded { generation, result } => {
            if generation != st.palette_gen {
                return;
            }
            let Some(Palette { mode: Mode::Pick(pick), selected, .. }) = st.palette.as_mut() else { return };
            if pick.kind != PickKind::FileHistory {
                return;
            }
            *selected = 0;
            match result {
                Ok(paths) => pick.set_items(Items::Ready(paths)),
                Err(e) => {
                    pick.set_items(Items::Failed(e.clone()));
                    st.log(false, e);
                }
            }
        }
        _ => {}
    }
}

/// Whether the palette may open now: not over a dialog, Preferences or a credential prompt.
fn may_open(st: &AppState) -> bool {
    st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing()
}

/// Switches the palette to the sub-list for `kind` (opening it if closed) and loads its items.
fn enter_pick(st: &mut AppState, kind: PickKind, fx: &mut Vec<Effect>) {
    if !palette::can_pick_file(st) {
        return;
    }
    let Some(workdir) = st.workdir() else { return };
    if st.palette.is_none() {
        if !may_open(st) {
            return;
        }
        st.palette = Some(Palette::default());
    }
    let Some(p) = st.palette.as_mut() else { return };
    p.mode = Mode::Pick(Pick::new(kind));
    p.query.clear();
    p.selected = 0;
    p.scroll_to_selected = false;
    st.palette_gen += 1;
    fx.push(Effect::LoadTrackedPaths { workdir, generation: st.palette_gen });
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
        C::OpenFileHistory(path) => Msg::OpenFileHistory { path, at: None },
        // Opens a sub-list in `handle`; never run directly.
        C::FileHistory => return,
    };
    fx.extend(super::update(st, msg, now));
}
