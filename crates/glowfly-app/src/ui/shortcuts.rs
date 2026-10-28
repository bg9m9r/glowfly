//! Keyboard shortcuts: every changeable key, and Go to tab 1 to 8, matched through the effective
//! keymap (`global.rs`, with presses turned into chords in `chords.rs`); recording a key for
//! Preferences > Keyboard (`record.rs`); the shortcut sheet (`sheet.rs`) and the key caps it
//! and Preferences draw (`caps.rs`). `update/shortcuts.rs` decides whether the staging keys
//! apply. "Ctrl" is Cmd on macOS, except where a chord says `ctrl`.

mod caps;
mod chords;
mod custom_keys;
mod global;
mod record;
mod sheet;
mod sheet_menus;

#[cfg(test)]
mod command_keys_tests;
#[cfg(test)]
mod fold_key_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod platform_tests;
#[cfg(test)]
mod quit_key_tests;
#[cfg(test)]
mod rebound_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod select_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tests;

pub(super) use caps::Caps;
pub(super) use global::input;
pub(super) use sheet::show as sheet;

/// Whether the widget with the keyboard focus is a text field.
pub(super) fn text_field_focused(ctx: &eframe::egui::Context) -> bool {
    ctx.memory(|m| m.focused()).is_some_and(|id| eframe::egui::text_edit::TextEditState::load(ctx, id).is_some())
}
