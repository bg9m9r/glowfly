//! Exploration tests: seeded random sessions on the whole-app journey harness. This is the pure
//! core they share; the driver, the checks and the tests that run sessions come later.
//!
//! - `rng`: the seeded generator (splitmix64).
//! - `action`: the action vocabulary and its one-line case text.
//! - `scene`: the fixture repositories built from a seed.
//! - `shadow`: the markers of live edits the session expects to find again.
//! - `reach`: whether markers and commit summaries are reachable in a repository.
//! - `probe`: whether a step's action took effect, from readings of the repository around it.

mod action;
mod probe;
mod reach;
mod rng;
mod scene;
mod shadow;
