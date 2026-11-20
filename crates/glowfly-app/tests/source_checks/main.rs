//! The source checks, one test binary: rules over every crate's sources and manifests that the
//! compiler can't enforce. `hygiene` ratchets file length, module docs, English text, copied
//! fixtures, harness builders and `cfg(test)` switches; `layering` keeps I/O out of the views
//! and the model; `i18n_keys` matches the code's translation keys with the English catalog;
//! `dependencies` keeps the crate graph pointing one way; `dead_pub` ratchets unused public items
//! in the library crates; `fn_length` keeps clippy's function-length cap on and its marked
//! functions only shrinking; `access_names` ratchets controls the view snapshots show without a
//! screen-reader name; `architecture_docs` checks that `docs/architecture/` names every source
//! file; `config_values` keeps git config entry values read through glowfly-core's
//! `config_entry`; `log_encoding` keeps the git commands that print commit text on `gitcli::utf8_args`;
//! `stash_actions` keeps the messages that act on one stash behind their builder;
//! `command_echo` keeps every `$ git …` echo of a job's output built by `text::command_echo`.
//! All of them read the sources through glowfly-ui's string-aware
//! `view_checks/support/lexer.rs`.

#[path = "../../../glowfly-ui/tests/view_checks/support/lexer.rs"]
mod lexer;
#[path = "../../../glowfly-ui/tests/view_checks/support/views.rs"]
mod views;

mod access_names;
mod architecture_docs;
mod command_echo;
mod config_values;
mod dead_pub;
mod dependencies;
mod fn_length;
mod hygiene;
mod i18n_keys;
mod layering;
mod log_encoding;
mod stash_actions;
