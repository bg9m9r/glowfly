//! Fuzz target: what a patch file holds, read without git (the Apply patch dialog).
//! The invariants it checks are in glowfly-core's `fuzzing::patch_summary`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::patch_summary(data));
