//! The source checks, one test binary: rules over every crate's sources and manifests that the
//! compiler can't enforce. `hygiene` ratchets file length, module docs, English log text, copied
//! fixtures, harness builders and `cfg(test)` switches; `layering` keeps I/O out of the views
//! and the model; `i18n_keys` matches the code's translation keys with the English catalog;
//! `dependencies` keeps the crate graph pointing one way.

#[path = "../../../glowfly-ui/tests/view_checks/support/views.rs"]
mod views;

mod dependencies;
mod hygiene;
mod i18n_keys;
mod layering;
