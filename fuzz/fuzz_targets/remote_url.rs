//! Fuzz target: which hosting service, host and repository a remote URL names.
//! The invariants it checks are in glowfly-core's `fuzzing::remote_url`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::remote_url(data));
