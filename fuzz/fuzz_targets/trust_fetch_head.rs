//! Fuzz target: FETCH_HEAD, read for the commits a pinned pull merges.
//! The invariants it checks are in glowfly-core's `fuzzing::trust_fetch_head`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::trust_fetch_head(data));
