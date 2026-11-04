//! Asking the runner, before a close that ends terminals (quitting, closing repository tabs,
//! Install and restart), what their shells are doing, and handing the busy ones to the close
//! that waits on the answer. The check is the one closing a terminal tab or Kill uses
//! (`terminal::ShellProbe::activity`), for several sessions in one answer.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::terminal::busy::busy_from;
use crate::terminal::{BusyCheck, TerminalEffect as E};
use glowfly_core::terminal::ShellActivity;
use std::time::Instant;

/// Asks what the shells of sessions `ids` are doing, for `then`.
pub(in crate::update) fn check(ids: Vec<u64>, then: BusyCheck, fx: &mut Vec<Effect>) {
    fx.push(Effect::Terminal(E::CheckBusy { ids, then }));
}

/// The runner's answer: the terminals that still run a program go to the close waiting on it,
/// which asks or goes ahead.
pub(in crate::update) fn answered(st: &mut AppState, found: &[(u64, ShellActivity)], then: BusyCheck, now: Instant, fx: &mut Vec<Effect>) {
    let busy = busy_from(st, found);
    match then {
        BusyCheck::Quit => crate::update::app::checked(st, busy, fx),
        BusyCheck::CloseTabs(req) => crate::update::tabs_close::checked(st, req, busy, fx),
        BusyCheck::Restart => crate::update::updates_restart::checked(st, busy, now, fx),
    }
}
