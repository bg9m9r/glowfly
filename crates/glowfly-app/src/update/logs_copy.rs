//! The logs panel's Copy entry (the row menu, the open row's Copy) and Copy log (the header's
//! More menu, the row menu). The text is built in state/activity_copy.rs.

use crate::effect::{Effect, UiEffect};
use crate::logs::LogKind;
use crate::state::AppState;
use crate::state::activity_copy::{entry_copy, log_text};
use crate::state::logs::EntryId;
use std::time::Instant;

/// Copies the entry `id` (a routine group's first entry: the whole group), without a toast like
/// the other Copy buttons. Nothing happens for an entry no longer in the log.
pub(super) fn copy_entry(st: &mut AppState, id: EntryId, fx: &mut Vec<Effect>) {
    if let Some(text) = entry_copy(&st.log, id, st.settings.lang(), st.view.utc_offset) {
        fx.push(Effect::Ui(UiEffect::CopyText(text)));
    }
}

/// Copies what the current tab shows and says so. Nothing happens while it can't (the menu item
/// is disabled then).
pub(super) fn copy_log(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some((text, n)) = log_text(st) else { return };
    fx.push(Effect::Ui(UiEffect::CopyText(text)));
    let lines = if n == 1 { "1 line".to_owned() } else { format!("{n} lines") };
    let said = match st.logs.tab {
        LogKind::Activity => {
            let entries = if n == 1 { "1 entry".to_owned() } else { format!("{n} entries") };
            format!("Copied the activity log to the clipboard ({entries})")
        }
        LogKind::Errors => format!("Copied {lines} of the error log to the clipboard"),
        LogKind::Performance => format!("Copied {lines} of the performance log to the clipboard"),
    };
    st.toast(true, said, now);
}

#[cfg(test)]
#[path = "logs_copy_tests.rs"]
mod tests;
