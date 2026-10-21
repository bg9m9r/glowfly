//! The one way view tests compare against an approved image. With `GLOWFLY_SKIP_SNAPSHOTS=1`
//! (CI on macOS and Linux, where the images were not rendered and wgpu may have no adapter) the
//! comparison is skipped with a note, and the rest of each test (driving the harness, checking
//! messages) still runs.

use egui_kittest::{Harness, SnapshotError};

pub(crate) const SKIP_VAR: &str = "GLOWFLY_SKIP_SNAPSHOTS";

/// How many pixels may differ from the approved image before a comparison fails. The approved
/// images were rendered on a developer machine; the Windows CI runner's renderer anti-aliases a
/// handful of edge pixels differently (12 to 14 per image), which says nothing about the layout.
/// A real change (a moved widget, a new label, a wrong colour) touches far more pixels than this.
pub(crate) const TOLERATED_PIXELS: i32 = 40;

/// Whether a comparison that found `diff` differing pixels still passes.
pub(crate) fn within_tolerance(diff: i32) -> bool {
    diff <= TOLERATED_PIXELS
}

/// Whether `value` (the variable's) asks for snapshots to be skipped: set to anything but
/// empty, `0` or `false`.
pub(crate) fn skip_requested(value: Option<&str>) -> bool {
    value.is_some_and(|v| !matches!(v.trim(), "" | "0" | "false"))
}

pub(crate) trait CheckSnapshot {
    /// `Harness::snapshot`, unless snapshots are skipped, passing when no more than
    /// `TOLERATED_PIXELS` differ.
    fn check_snapshot(&mut self, name: &str);
}

impl<S> CheckSnapshot for Harness<'_, S> {
    #[track_caller]
    fn check_snapshot(&mut self, name: &str) {
        if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
            eprintln!("note: {SKIP_VAR} is set; not comparing snapshot {name}");
            return;
        }
        match self.try_snapshot(name) {
            Ok(()) => {}
            Err(SnapshotError::Diff { diff, .. }) if within_tolerance(diff) => {
                eprintln!("note: snapshot {name} differs in {diff} pixels, within the tolerated {TOLERATED_PIXELS}");
            }
            Err(err) => panic!("{err}"),
        }
    }
}

#[test]
fn tolerance() {
    assert!(within_tolerance(0));
    assert!(within_tolerance(14), "the CI renderer's usual difference");
    assert!(within_tolerance(TOLERATED_PIXELS));
    assert!(!within_tolerance(TOLERATED_PIXELS + 1));
}

#[test]
fn skip_values() {
    for v in ["1", "true", "yes"] {
        assert!(skip_requested(Some(v)), "{v}");
    }
    for v in [None, Some(""), Some("0"), Some("false"), Some(" ")] {
        assert!(!skip_requested(v), "{v:?}");
    }
}
