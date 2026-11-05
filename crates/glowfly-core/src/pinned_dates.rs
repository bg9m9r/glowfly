//! The two places a fixture folder's pinned dates ([`crate::testkit::clock`]) reach production
//! code: git commands started by gitcli and Glowfly's own signatures. Test and `testkit` builds
//! use the clock's versions; a release build gets these stand-ins, which change nothing.

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
}

// The explicit imports below take the place of the glob's when the clock is built in.
#[allow(unused_imports)]
pub(crate) use unpinned::*;

#[cfg(any(test, feature = "testkit"))]
pub(crate) use crate::testkit::clock::{apply, signature};
