//! A palette command run from its key (Preferences > Keyboard): the same message the palette
//! entry sends, only while the palette would list the entry.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::palette::{self, PaletteCommand};
use crate::state::palette_bindings;
use std::time::Instant;

/// Runs `cmd` as its palette entry would, or opens its step for a two-step command; one-step
/// commands leave the palette closed, and nothing is recorded among recent commands. When the
/// palette wouldn't list `cmd` now, the toolbar's reason for the same button goes to the status
/// bar, or nothing happens when it has none.
pub(super) fn run_bound(st: &mut AppState, cmd: PaletteCommand, now: Instant, fx: &mut Vec<Effect>) {
    if !palette::entries(st).iter().any(|e| e.command == cmd) {
        if let Some(why) = palette_bindings::blocked(st, &cmd) {
            st.log(false, why);
        }
        return;
    }
    if let Some(kind) = cmd.pick_kind() {
        return super::palette::enter_pick(st, kind, fx);
    }
    if let Some(kind) = cmd.input_kind() {
        return super::palette::enter_input(st, kind);
    }
    super::palette::run(st, cmd, now, fx);
}
