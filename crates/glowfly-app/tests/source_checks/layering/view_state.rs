//! Views take `&AppState` and the `&mut ViewState` that `AppState::with_view` lends them, so the
//! compiler keeps them from changing the model (#370). Those that still take `&mut AppState`
//! write a part of the model in place (a text buffer or form, a panel's scroll, the terminal's
//! selection, Preferences' settings). Their number in glowfly-ui's non-test sources only
//! shrinks: lower `MUT_STATE_VIEWS` when one moves to `ViewState` or a `Msg`, never raise it.

use crate::lexer::bare_lines;
use crate::views::view_sources;

/// The `&mut AppState` parameters left in the views.
const MUT_STATE_VIEWS: usize = 39;

#[test]
fn fewer_views_take_the_whole_state() {
    let mut found = Vec::new();
    for (rel, text) in view_sources() {
        for (i, line) in bare_lines(&text).iter().enumerate() {
            for _ in line.matches("&mut AppState") {
                found.push(format!("crates/glowfly-ui/src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        found.len() <= MUT_STATE_VIEWS,
        "a view took `&mut AppState` again: give it `st: &AppState, view: &mut ViewState` (lent by \
         `AppState::with_view`) and push a Msg for anything else it changes:\n{}",
        found.join("\n")
    );
    assert!(found.len() == MUT_STATE_VIEWS, "fewer views take `&mut AppState` now: lower MUT_STATE_VIEWS to {}", found.len());
}
