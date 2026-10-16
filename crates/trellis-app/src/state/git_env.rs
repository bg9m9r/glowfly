//! The git Trellis runs, as `update` and the views see it: found and asked its version once at
//! startup, then shown in Preferences and, while it is missing or too old, in a banner.

use trellis_core::git_version::{Feature, GitInfo};

#[derive(Default)]
pub struct GitState {
    /// None until the startup check answers.
    pub info: Option<GitInfo>,
    /// The banner was closed (until Trellis restarts).
    pub banner_dismissed: bool,
}

impl GitState {
    /// Whether git has `f`; true while unknown, so nothing is held up before the check answers
    /// (git's own error explains a failure then).
    pub fn has(&self, f: Feature) -> bool {
        self.info.as_ref().is_none_or(|i| i.has(f))
    }

    /// Why `f` is off ("Sparse checkout needs git 2.35 or newer (you have 2.30.1)."), or None.
    pub fn blocked(&self, f: Feature) -> Option<String> {
        (!self.has(f)).then(|| f.needs_text(self.info.as_ref().and_then(|i| i.version)))
    }

    /// The banner's text, unless it was closed or all is well.
    pub fn banner(&self) -> Option<String> {
        if self.banner_dismissed {
            return None;
        }
        self.info.as_ref().and_then(GitInfo::banner)
    }
}
