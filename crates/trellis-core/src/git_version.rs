//! Which git Trellis runs and what it can do. Checked at startup and again whenever another git
//! is chosen in Preferences > Git: the program ([`crate::git_program`]: the chosen one, else the
//! one found on the PATH children get) and its version, and from that the features that need a
//! newer git than the one installed, which are then turned off with a reason instead of failing
//! with git's usage text.

use crate::gitcli;
use std::path::{Path, PathBuf};

/// A git version: (major, minor, patch).
pub type Version = (u32, u32, u32);

/// A feature that needs a git newer than some still in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// `git restore`: discarding changes to a file, restoring a file from a commit.
    Restore,
    /// `git push --force-if-includes`: force pushing only over commits seen locally.
    ForceIfIncludes,
    /// `git sparse-checkout set --cone/--no-cone --no-sparse-index --stdin`.
    SparseCheckout,
    /// `git merge-tree --write-tree`: predicting merge conflicts.
    MergeTree,
}

impl Feature {
    pub const ALL: [Feature; 4] = [Feature::Restore, Feature::ForceIfIncludes, Feature::SparseCheckout, Feature::MergeTree];

    /// The oldest git (major, minor) that has it.
    pub fn min(self) -> (u32, u32) {
        match self {
            Feature::Restore => (2, 23),
            Feature::ForceIfIncludes => (2, 30),
            // `set` learned `--cone`/`--no-cone` and the sparse-index options in 2.35.
            Feature::SparseCheckout => (2, 35),
            Feature::MergeTree => (2, 38),
        }
    }

    /// What it does, for "needs git 2.38 or newer" texts.
    pub fn label(self) -> &'static str {
        match self {
            Feature::Restore => "discarding changes to a file",
            Feature::ForceIfIncludes => "force pushing only over commits you have seen",
            Feature::SparseCheckout => "sparse checkout",
            Feature::MergeTree => "predicting merge conflicts",
        }
    }

    /// "Sparse checkout needs git 2.35 or newer (you have 2.30.1)."
    pub fn needs_text(self, have: Option<Version>) -> String {
        let (major, minor) = self.min();
        let mut what = self.label().to_owned();
        if let Some(first) = what.get(..1) {
            what = first.to_uppercase() + &what[1..];
        }
        match have {
            Some(v) => format!("{what} needs git {major}.{minor} or newer (you have {}).", show(v)),
            None => format!("{what} needs git {major}.{minor} or newer."),
        }
    }
}

/// The git every feature works with: the newest of the features' minimums.
pub fn full_support() -> (u32, u32) {
    Feature::ALL.iter().map(|f| f.min()).max().unwrap_or((0, 0))
}

/// "2.39.3".
pub fn show(v: Version) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// The features `version` lacks, in [`Feature::ALL`]'s order. An unknown version (git didn't run
/// or printed something unexpected) lacks nothing: commands then fail with git's own error.
pub fn missing_for(version: Option<Version>) -> Vec<Feature> {
    let Some(v) = version else { return Vec::new() };
    Feature::ALL.into_iter().filter(|f| (v.0, v.1) < f.min()).collect()
}

/// What is known about the git Trellis runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitInfo {
    /// Where the `git` program is: the chosen one, or the one found on PATH.
    pub path: Option<PathBuf>,
    /// Its version, when it ran and said.
    pub version: Option<Version>,
    /// The program was chosen in Preferences > Git rather than found on PATH.
    pub chosen: bool,
}

impl GitInfo {
    /// Whether this git has `f` (an unknown version counts as having everything).
    pub fn has(&self, f: Feature) -> bool {
        !missing_for(self.version).contains(&f)
    }

    /// The features this git lacks.
    pub fn missing(&self) -> Vec<Feature> {
        missing_for(self.version)
    }

    /// The banner shown while something is wrong with git: it can't be run, or it is too old
    /// for some features (each named). None when all is well.
    pub fn banner(&self) -> Option<String> {
        let (major, minor) = full_support();
        let Some(v) = self.version else {
            return Some(match &self.path {
                Some(p) if self.chosen => format!(
                    "Trellis couldn't run git at {}, the git chosen in Preferences > Git. Choose another there.",
                    crate::native_path::shown(p)
                ),
                Some(p) => format!("Trellis couldn't run git at {}. Commits, fetches and pushes need it.", crate::native_path::shown(p)),
                None => "Trellis couldn't find git on your PATH. Commits, fetches and pushes need it: install git, then choose it in Preferences > Git or restart Trellis."
                    .to_owned(),
            });
        };
        let missing = self.missing();
        if missing.is_empty() {
            return None;
        }
        let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
        Some(format!(
            "Your git is {}; Trellis needs {major}.{minor} or newer for everything to work. Until you update git, these are off: {}.",
            show(v),
            names.join(", ")
        ))
    }
}

/// Which git runs (the one chosen in Preferences > Git, else the one looked up on the PATH
/// children get) and its version, asked in `cwd`.
pub fn detect(cwd: &Path) -> GitInfo {
    if let Some(p) = crate::git_program::chosen() {
        let version = crate::git_program::probe(&p, cwd).ok();
        return GitInfo { path: Some(p), version, chosen: true };
    }
    GitInfo { path: crate::external_tools::find_on_path(Path::new("git")), version: gitcli::version(cwd), chosen: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_follow_the_version() {
        assert_eq!(missing_for(Some((2, 22, 9))), Feature::ALL.to_vec());
        assert_eq!(missing_for(Some((2, 23, 0))), [Feature::ForceIfIncludes, Feature::SparseCheckout, Feature::MergeTree]);
        assert_eq!(missing_for(Some((2, 34, 1))), [Feature::SparseCheckout, Feature::MergeTree]);
        assert_eq!(missing_for(Some((2, 37, 7))), [Feature::MergeTree]);
        assert!(missing_for(Some((2, 38, 0))).is_empty());
        assert!(missing_for(Some((3, 0, 0))).is_empty());
        assert!(missing_for(None).is_empty(), "unknown: let git say what fails");
        assert_eq!(full_support(), (2, 38));
        assert_eq!(full_support(), crate::conflict_check::MIN_GIT);
    }

    #[test]
    fn the_banner_names_the_minimum_and_what_is_off() {
        let old = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 34, 1)), chosen: false };
        let b = old.banner().unwrap();
        assert!(b.contains("2.34.1") && b.contains("2.38 or newer") && b.contains("sparse checkout, predicting merge conflicts"), "{b}");
        assert!(!old.has(Feature::MergeTree) && old.has(Feature::Restore));
        assert_eq!(GitInfo { path: None, version: Some((2, 45, 0)), chosen: false }.banner(), None);
        let none = GitInfo::default().banner().unwrap();
        assert!(none.contains("couldn't find git") && none.contains("Preferences > Git"), "{none}");
        assert!(!none.contains("  "), "no runs of spaces in the text: {none}");
        let broken = GitInfo { path: Some("/x/git".into()), version: None, chosen: false }.banner().unwrap();
        assert!(broken.contains("couldn't run git") && !broken.contains("Preferences > Git"), "{broken}");
        let chosen = GitInfo { path: Some("/x/git".into()), version: None, chosen: true }.banner().unwrap();
        assert!(chosen.contains("couldn't run git") && chosen.contains("chosen in Preferences > Git"), "{chosen}");
        assert_eq!(Feature::SparseCheckout.needs_text(Some((2, 30, 1))), "Sparse checkout needs git 2.35 or newer (you have 2.30.1).");
    }

    #[test]
    fn this_git_is_found_and_has_a_version() {
        let info = detect(&crate::paths::temp_dir());
        assert!(info.version.is_some(), "{info:?}");
        let path = info.path.expect("git is on PATH");
        assert!(path.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("git")), "{}", path.display());
    }
}
