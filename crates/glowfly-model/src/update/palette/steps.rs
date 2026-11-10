//! Entering the palette's second steps: the sub-lists a two-step command opens (tracked files,
//! stashes, profiles, issues, branches and tags to switch to, tags to delete) and its free-text
//! steps, opening the palette first when it is closed.

use super::may_open;
use crate::effect::{Effect, FilesEffect};
use crate::state::AppState;
use crate::state::palette::{self, InputKind, Mode, Palette, Pick, PickKind};

/// Switches the palette to `mode` with an empty query, opening it if closed (unless something
/// covers the window). Returns whether the step opened.
fn open_step(st: &mut AppState, mode: Mode) -> bool {
    if st.palette.is_none() {
        if !may_open(st) {
            return false;
        }
        st.palette = Some(Palette::default());
        st.view.palette_list = Default::default();
    }
    let Some(p) = st.palette.as_mut() else { return false };
    p.mode = mode;
    p.query.clear();
    p.selected = 0;
    st.view.palette_scroll = false;
    st.palette_gen += 1;
    true
}

/// Switches the palette to the sub-list for `kind` (opening it if closed) and fills it: the
/// tracked files load in the background, the stashes and checkouts come from the snapshot, the
/// profiles from settings.
pub(in crate::update) fn enter_pick(st: &mut AppState, kind: PickKind, fx: &mut Vec<Effect>) {
    match kind {
        PickKind::FileHistory | PickKind::Blame => {
            if !palette::can_pick_file(st) {
                return;
            }
            let Some(workdir) = st.workdir() else { return };
            if open_step(st, Mode::Pick(Pick::new(kind))) {
                fx.push(Effect::Files(FilesEffect::LoadTracked { workdir, generation: st.palette_gen }));
            }
        }
        PickKind::Stash(action) => {
            if !palette::can_pick_stash(st) {
                return;
            }
            let list = st.snap.as_ref().map(|s| s.stashes.clone()).unwrap_or_default();
            open_step(st, Mode::Pick(Pick::stashes(action, &list)));
        }
        PickKind::RenameStash => {
            if !palette::can_pick_stash(st) {
                return;
            }
            let list = st.snap.as_ref().map(|s| s.stashes.clone()).unwrap_or_default();
            open_step(st, Mode::Pick(Pick::stashes_to_rename(&list)));
        }
        PickKind::Profile => {
            if st.settings.profiles.len() < 2 || st.profile_switch.is_some() {
                return;
            }
            let pick = Pick::profiles(&st.settings);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::GitFlowIssue(kind) => {
            if !crate::state::gitflow::issues_ready(st) {
                return;
            }
            let Some(f) = st.session.forge.as_ref() else { return };
            let pick = Pick::issues(kind, f);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::ViewIssue => {
            if !crate::state::palette::commands::issues::can_view(st) {
                return;
            }
            let Some(f) = st.session.forge.as_ref() else { return };
            let pick = Pick::issues_to_view(f);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::SwitchBranch => {
            let Some(snap) = st.snap.as_ref() else { return };
            let pick = Pick::checkouts(snap);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::DeleteTag => {
            let Some(snap) = st.snap.as_ref().filter(|s| !s.tags.is_empty()) else { return };
            let pick = Pick::tags_to_delete(snap);
            open_step(st, Mode::Pick(pick));
        }
    }
}

/// Switches the palette to the free-text step `kind` (opening it if closed).
pub(in crate::update) fn enter_input(st: &mut AppState, kind: InputKind) {
    let ok = match kind {
        InputKind::StashMessage => palette::can_stash(st),
        InputKind::DeepenBy => st.can_fetch_history(),
    };
    if ok {
        open_step(st, Mode::Input(kind));
    }
}
