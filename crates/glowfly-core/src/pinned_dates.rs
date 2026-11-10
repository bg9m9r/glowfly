//! The places a fixture folder's pinned dates ([`crate::testkit::clock`]) reach production
//! code: git commands started by gitcli, Glowfly's own signatures, and the "now" that ages the
//! records those signatures date. Test and `testkit` builds use the clock's versions; a release
//! build gets these stand-ins, which change nothing.

use git2::{Error, Signature};
use std::path::Path;
use std::process::Command;

#[allow(dead_code)] // shadowed in test builds
mod unpinned {
    use super::*;

    pub(crate) fn apply(_: &mut Command, _: &Path, _: &[(&str, &str)]) {}

    pub(crate) fn signature(_: &Path, name: &str, email: &str) -> Result<Signature<'static>, Error> {
        Signature::now(name, email)
    }

    pub(crate) fn now(_: &Path) -> i64 {
        super::real_now()
    }
}

/// The real clock, in Unix seconds.
pub(crate) fn real_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

// The explicit imports below take the place of the glob's when the clock is built in.
#[allow(unused_imports)]
pub(crate) use unpinned::*;

#[cfg(any(test, feature = "testkit"))]
pub(crate) use crate::testkit::clock::{apply, now, signature};
