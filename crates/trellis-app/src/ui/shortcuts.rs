//! Keyboard shortcuts: every changeable key, and Go to tab 1 to 8, matched through the effective
//! keymap (`global.rs`, with presses turned into chords in `chords.rs`); recording a key for
//! Preferences > Keyboard (`record.rs`); the shortcut sheet (`sheet.rs`) and the key caps it
//! and Preferences draw (`caps.rs`). `update/shortcuts.rs` decides whether the staging keys
//! apply. "Ctrl" is Cmd on macOS, except where a chord says `ctrl`.

mod caps;
mod chords;
mod global;
mod record;
mod sheet;

#[cfg(test)]
mod platform_tests;
#[cfg(test)]
mod rebound_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;

pub(super) use caps::Caps;
pub(super) use global::input;
pub(super) use sheet::show as sheet;
