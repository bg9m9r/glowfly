//! The Applying patches banner on the WIP row while a `git am` series is stopped: its title
//! (which patch of how many), the stopped patch and what to do next, and its button labels.
//! Pure.

use trellis_core::patch_ops::AmProgress;

pub const AM_NAME: &str = "Applying patches";
pub const AM_CONTINUE: &str = "Continue";
pub const AM_SKIP: &str = "Skip patch";
pub const AM_ABORT: &str = "Abort";
pub const HINT_CONFLICTS: &str = "Resolve the conflicted files, then Continue.";
pub const HINT_STAGED: &str = "Continue commits this patch and applies the rest.";
pub const HINT_BY_HAND: &str = "This patch didn't apply. Make its changes by hand and stage them, then Continue, or Skip patch.";

#[derive(Clone, Debug, PartialEq)]
pub struct AmBannerText {
    /// "Applying patches · patch 2 of 3".
    pub title: String,
    /// "Stopped at: <subject>", when the subject is known.
    pub stop: Option<String>,
    pub hint: &'static str,
}

/// The banner's text for `p`, with `conflicts_left` whether conflicted files remain and
/// `staged` whether anything is staged.
pub fn lines(p: &AmProgress, conflicts_left: bool, staged: bool) -> AmBannerText {
    let title = if p.total == 0 { AM_NAME.to_owned() } else { format!("{AM_NAME} · patch {} of {}", p.step, p.total) };
    let stop = (!p.subject.is_empty()).then(|| format!("Stopped at: {}", p.subject));
    let hint = if conflicts_left {
        HINT_CONFLICTS
    } else if staged {
        HINT_STAGED
    } else {
        HINT_BY_HAND
    };
    AmBannerText { title, stop, hint }
}

/// Whether a job labelled `l` is one of the banner's buttons.
pub fn is_am_label(l: &str) -> bool {
    [AM_CONTINUE, AM_SKIP, AM_ABORT].contains(&l)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(total: usize, subject: &str) -> AmProgress {
        AmProgress { step: 2, total, subject: subject.into(), conflicted: false }
    }

    #[test]
    fn lines_for_each_stop() {
        let b = lines(&progress(3, "Fix the form"), true, true);
        assert_eq!(b.title, "Applying patches · patch 2 of 3");
        assert_eq!(b.stop.as_deref(), Some("Stopped at: Fix the form"));
        assert_eq!(b.hint, HINT_CONFLICTS);
        assert_eq!(lines(&progress(3, "x"), false, true).hint, HINT_STAGED);
        assert_eq!(lines(&progress(3, "x"), false, false).hint, HINT_BY_HAND);
        let b = lines(&progress(0, ""), false, false);
        assert_eq!((b.title.as_str(), b.stop), ("Applying patches", None));
    }

    #[test]
    fn button_labels() {
        assert!(is_am_label("Continue") && is_am_label("Skip patch") && is_am_label("Abort"));
        assert!(!is_am_label("Abort rebase") && !is_am_label("Apply patches"));
    }
}
