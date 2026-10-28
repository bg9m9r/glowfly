//! The undo tests' stand-in Recycle Bin / Trash (`testkit::bin`): a folder in the temp dir, one
//! per process, with the `Trashers` that use it.

use super::Trashers;
use crate::repo::{TrashRefusal, Untrashed};
use std::path::{Path, PathBuf};

pub(super) fn bin() -> PathBuf {
    crate::testkit::bin::own("glowfly-undo-delete-bin")
}

pub(super) fn to_bin(full: &Path) -> Result<(), TrashRefusal> {
    crate::testkit::bin::move_in(&bin(), full)
}

pub(super) fn from_bin(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    crate::testkit::bin::take_out(&bin(), full, ours)
}

/// A trash that can't be read back, as on macOS.
pub(super) fn unreadable(_: &Path, _: &dyn Fn(&Path) -> bool) -> Untrashed {
    Untrashed::Unsupported
}

pub(super) fn never(_: &Path) -> Result<(), TrashRefusal> {
    panic!("this delete doesn't use the trash")
}

pub(super) const T: Trashers = Trashers { system: to_bin, home: never, restore: from_bin };

/// How many files in the stand-in bin came from `name` in the test's own repository `dir`.
pub(super) fn in_bin(dir: &Path, name: &str) -> usize {
    crate::testkit::bin::count(&bin(), dir, name)
}
