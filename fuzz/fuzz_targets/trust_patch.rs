//! Fuzz target: a patch's paths and whether they reach a hooks folder (the trust check for am and apply).
//! The invariants it checks are in glowfly-core's `fuzzing::trust_patch`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::trust_patch(data));
