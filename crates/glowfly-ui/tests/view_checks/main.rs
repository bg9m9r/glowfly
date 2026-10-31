//! glowfly-ui's view checks, one binary: source checks that keep the views on the shared
//! controls, type scale, design tokens, menu style, message styles, sentence case and dialog
//! fields. `support/` reads the sources (the app crate's `hygiene.rs` and `i18n_keys.rs` include
//! `support/views.rs` too).

mod controls;
mod dialog_fields;
mod menus;
mod message_styles;
mod sentence_case;
mod support;
mod tokens;
mod type_scale;
