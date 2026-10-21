//! The Applying patches banner on the WIP row while a `git am` series is stopped: the stopped
//! patch and what to do next (the title, which patch of how many, is
//! `op_actions::banner_title`), and its buttons' English labels, the ids the jobs run under.
//! Pure.

use glowfly_core::i18n::Lang;
use glowfly_core::patch_ops::AmProgress;

pub const AM_CONTINUE: &str = "Continue";
pub const AM_SKIP: &str = "Skip patch";
pub const AM_ABORT: &str = "Abort";

#[derive(Clone, Debug, PartialEq)]
pub struct AmBannerText {
    /// "Stopped at: <subject>", when the subject is known.
    pub stop: Option<String>,
    pub hint: &'static str,
}

/// The banner's text for `p`, with `conflicts_left` whether conflicted files remain and
/// `staged` whether anything is staged.
pub fn lines(t: Lang, p: &AmProgress, conflicts_left: bool, staged: bool) -> AmBannerText {
    let stop = (!p.subject.is_empty()).then(|| t.trf("details.banner.am.stopped", &[("subject", &p.subject)]));
    let hint = if conflicts_left {
        t.tr("details.banner.am.hint.conflicts")
    } else if staged {
        t.tr("details.banner.am.hint.staged")
    } else {
        t.tr("details.banner.am.hint.by_hand")
    };
    AmBannerText { stop, hint }
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
        let en = Lang::En;
        let b = lines(en, &progress(3, "Fix the form"), true, true);
        assert_eq!(b.stop.as_deref(), Some("Stopped at: Fix the form"));
        assert_eq!(b.hint, en.tr("details.banner.am.hint.conflicts"));
        assert_eq!(b.hint, "Resolve the conflicted files, then Continue.");
        assert_eq!(lines(en, &progress(3, "x"), false, true).hint, en.tr("details.banner.am.hint.staged"));
        assert_eq!(lines(en, &progress(3, "x"), false, false).hint, en.tr("details.banner.am.hint.by_hand"));
        assert_eq!(lines(en, &progress(0, ""), false, false).stop, None);
        let de = lines(Lang::De, &progress(3, "Fix the form"), false, false);
        assert!(de.stop.unwrap().contains("Fix the form"));
        assert_ne!(de.hint, en.tr("details.banner.am.hint.by_hand"));
    }

    #[test]
    fn button_labels() {
        assert!(is_am_label("Continue") && is_am_label("Skip patch") && is_am_label("Abort"));
        assert!(!is_am_label("Abort rebase") && !is_am_label("Apply patches"));
    }
}
