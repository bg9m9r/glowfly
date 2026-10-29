//! Fuzz target: conflict markers in a working file, and merged text split into clean runs and chunks.
//! The invariants it checks are in glowfly-core's `fuzzing::conflict_markers`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::conflict_markers(data));
