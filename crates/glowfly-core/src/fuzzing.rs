//! The checks the fuzz targets in `fuzz/` run, built only for tests and with the `fuzzing`
//! feature. Each takes the fuzzer's bytes, feeds them to a parser of text from other people
//! (a rebase todo, FETCH_HEAD, a patch, a remote URL, merged text with conflict markers), and
//! panics when the parser panics or breaks an invariant written beside it. The unit tests
//! (fuzzing/tests.rs) run every check over the seeds in `fuzz/seeds/` and mutations of them, so
//! an ordinary `cargo test` builds and exercises them too.

mod diffs;
mod trust;
mod urls;

pub use diffs::{conflict_markers, patch_summary};
pub use trust::{trust_fetch_head, trust_patch, trust_todo};
pub use urls::{redact_credentials, remote_url};

/// A check: panics when the parser it feeds breaks an invariant.
pub type Check = fn(&[u8]);

/// Every check, by the name of its fuzz target.
pub const CHECKS: [(&str, Check); 7] = [
    ("conflict_markers", conflict_markers),
    ("patch_summary", patch_summary),
    ("redact_credentials", redact_credentials),
    ("remote_url", remote_url),
    ("trust_fetch_head", trust_fetch_head),
    ("trust_patch", trust_patch),
    ("trust_todo", trust_todo),
];

/// The fuzzer's bytes as text, read lossily so that every input reaches the parser.
fn text(data: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(data)
}

#[cfg(test)]
mod tests;
