//! The one way view tests compare against an approved image. With `TRELLIS_SKIP_SNAPSHOTS=1`
//! (CI on macOS and Linux, where the images were not rendered and wgpu may have no adapter) the
//! comparison is skipped with a note, and the rest of each test (driving the harness, checking
//! messages) still runs.

use egui_kittest::Harness;

pub(crate) const SKIP_VAR: &str = "TRELLIS_SKIP_SNAPSHOTS";

/// Whether `value` (the variable's) asks for snapshots to be skipped: set to anything but
/// empty, `0` or `false`.
pub(crate) fn skip_requested(value: Option<&str>) -> bool {
    value.is_some_and(|v| !matches!(v.trim(), "" | "0" | "false"))
}

pub(crate) trait CheckSnapshot {
    /// `Harness::snapshot`, unless snapshots are skipped.
    fn check_snapshot(&mut self, name: &str);
}

impl<S> CheckSnapshot for Harness<'_, S> {
    #[track_caller]
    #[allow(clippy::disallowed_methods)]
    fn check_snapshot(&mut self, name: &str) {
        if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
            eprintln!("note: {SKIP_VAR} is set; not comparing snapshot {name}");
            return;
        }
        self.snapshot(name);
    }
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
