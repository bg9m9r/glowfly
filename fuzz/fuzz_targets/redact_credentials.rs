//! Fuzz target: the redaction of passwords in URLs before a command is logged.
//! The invariants it checks are in glowfly-core's `fuzzing::redact_credentials`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| glowfly_core::fuzzing::redact_credentials(data));
