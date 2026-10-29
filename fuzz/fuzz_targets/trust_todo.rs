//! Fuzz target: a rebase or sequencer todo, read for the commits continuing it brings in.
//! The invariants it checks are in glowfly-core's `fuzzing::trust_todo`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::trust_todo(data));
