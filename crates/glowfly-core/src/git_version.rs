//! Which git Glowfly runs and what it can do. Checked at startup and again whenever another git
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
    /// `git range-diff`: comparing two versions of a branch commit by commit.
    RangeDiff,
    /// `git restore`: discarding changes to a file, restoring a file from a commit.
    Restore,
    /// `git submodule set-url` (2.25) and `set-branch` (2.22): editing a submodule.
    SubmoduleEdit,
    /// `git push --force-if-includes`: force pushing only over commits seen locally.
    ForceIfIncludes,
    /// `git sparse-checkout set --cone/--no-cone --no-sparse-index --stdin`.
    SparseCheckout,
    /// `git merge-tree --write-tree`: predicting merge conflicts.
    MergeTree,
    /// `git cherry-pick --skip` and `git revert --skip`: skipping the commit a sequence stopped at.
    SequencerSkip,
    /// `git fetch --no-write-fetch-head` (2.29): downloading the file contents a partial clone
    /// is missing.
    FetchObjects,
    /// `git rebase --update-refs`: restacking the branches stacked on a rewritten branch.
    UpdateRefs,
}

impl Feature {
    pub const ALL: [Feature; 9] = [
        Feature::RangeDiff,
        Feature::Restore,
        Feature::SequencerSkip,
        Feature::SubmoduleEdit,
        Feature::FetchObjects,
        Feature::ForceIfIncludes,
        Feature::SparseCheckout,
        Feature::MergeTree,
        Feature::UpdateRefs,
    ];

    /// The oldest git (major, minor) that has it.
    pub fn min(self) -> (u32, u32) {
        match self {
            Feature::RangeDiff => (2, 19),
            Feature::Restore => (2, 23),
            Feature::SubmoduleEdit => (2, 25),
            Feature::ForceIfIncludes => (2, 30),
            // `set` learned `--cone`/`--no-cone` and the sparse-index options in 2.35.
            Feature::SparseCheckout => (2, 35),
            Feature::MergeTree => (2, 38),
            Feature::SequencerSkip => (2, 23),
            Feature::FetchObjects => (2, 29),
            Feature::UpdateRefs => (2, 38),
        }
    }

    /// What it does, for "needs git 2.38 or newer" texts.
    pub fn label(self) -> &'static str {
        match self {
            Feature::RangeDiff => "comparing two versions of a branch",
            Feature::Restore => "discarding changes to a file",
            Feature::SubmoduleEdit => "editing a submodule's URL and branch",
            Feature::ForceIfIncludes => "force pushing only over commits you have seen",
            Feature::SparseCheckout => "sparse checkout",
            Feature::MergeTree => "predicting merge conflicts",
            Feature::SequencerSkip => "skipping a commit in a cherry-pick or revert",
            Feature::FetchObjects => "downloading file contents a partial clone is missing",
            Feature::UpdateRefs => "restacking branches",
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

/// The oldest of the features' minimums. A git older than this has every gated feature off,
/// so it isn't offered as a fallback when the chosen git can't run.
pub fn oldest_gated() -> (u32, u32) {
    Feature::ALL.iter().map(|f| f.min()).min().unwrap_or((0, 0))
}

/// Whether `v` is at least [`oldest_gated`]: new enough to offer as a fallback.
pub fn new_enough(v: Version) -> bool {
    (v.0, v.1) >= oldest_gated()
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

/// What is known about the git Glowfly runs.
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

    /// What is wrong with git, in full: it can't be run, or it is too old for some features
    /// (each named). For the activity log and the welcome step. None when all is well.
    pub fn problem(&self) -> Option<String> {
        let (major, minor) = full_support();
        let Some(v) = self.version else { return Some(self.unusable_text()) };
        let missing = self.missing();
        if missing.is_empty() {
            return None;
        }
        let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
        Some(format!(
            "Your git is {}; Glowfly needs {major}.{minor} or newer for everything to work. Until you update git, these are off: {}.",
            show(v),
            names.join(", ")
        ))
    }

    /// The banner shown while something is wrong with git, in one sentence: it can't be run
    /// (as [`GitInfo::problem`] says it), or it is too old, naming up to three of the features
    /// that are off. None when all is well.
    pub fn banner(&self) -> Option<String> {
        let Some(v) = self.version else { return Some(self.unusable_text()) };
        let missing = self.missing();
        let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
        (!names.is_empty()).then(|| too_old_text(v, &names))
    }

    /// Why git can't be used: not found, or it didn't run (the chosen one, or the PATH one).
    fn unusable_text(&self) -> String {
        match &self.path {
            Some(p) if self.chosen => format!(
                "Glowfly couldn't run git at {}, the git chosen in Preferences > Git. Choose another there.",
                crate::native_path::shown(p)
            ),
            Some(p) => format!("Glowfly couldn't run git at {}. Commits, fetches and pushes need it.", crate::native_path::shown(p)),
            None => "Glowfly couldn't find git on your PATH. Commits, fetches and pushes need it: install git, then choose it in Preferences > Git or restart Glowfly."
                .to_owned(),
        }
    }
}

/// "Git 2.34.1 is older than 2.38, so … are off.", naming the features when there are at most
/// three of them.
fn too_old_text(v: Version, names: &[&str]) -> String {
    let (major, minor) = full_support();
    match names.len() {
        n @ 0..=3 => {
            let verb = if n == 1 { "is" } else { "are" };
            format!("Git {} is older than {major}.{minor}, so {} {verb} off.", show(v), and_list(names))
        }
        n => format!("Git {} is older than {major}.{minor}, so {n} features are off. Preferences > Git lists them.", show(v)),
    }
}

/// "a", "a and b", "a, b and c".
fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The Git project's download page for `os`, which says how to install or update git there.
pub fn download_url_for(os: crate::opens::Os) -> &'static str {
    match os {
        crate::opens::Os::Windows => "https://git-scm.com/download/win",
        crate::opens::Os::MacOs => "https://git-scm.com/download/mac",
        // That page also covers the other Unix systems.
        crate::opens::Os::Linux => "https://git-scm.com/download/linux",
    }
}

/// The Git project's download page for this OS.
pub fn download_url() -> &'static str {
    download_url_for(crate::opens::Os::current())
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
        assert_eq!(missing_for(Some((2, 18, 5))), Feature::ALL.to_vec());
        assert_eq!(missing_for(Some((2, 22, 9))), Feature::ALL[1..].to_vec());
        assert_eq!(
            missing_for(Some((2, 23, 0))),
            [
                Feature::SubmoduleEdit,
                Feature::FetchObjects,
                Feature::ForceIfIncludes,
                Feature::SparseCheckout,
                Feature::MergeTree,
                Feature::UpdateRefs
            ]
        );
        assert!(missing_for(Some((2, 24, 9))).contains(&Feature::SubmoduleEdit));
        assert_eq!(
            missing_for(Some((2, 25, 0))),
            [Feature::FetchObjects, Feature::ForceIfIncludes, Feature::SparseCheckout, Feature::MergeTree, Feature::UpdateRefs]
        );
        assert_eq!(
            missing_for(Some((2, 29, 0))),
            [Feature::ForceIfIncludes, Feature::SparseCheckout, Feature::MergeTree, Feature::UpdateRefs]
        );
        assert_eq!(missing_for(Some((2, 34, 1))), [Feature::SparseCheckout, Feature::MergeTree, Feature::UpdateRefs]);
        assert_eq!(missing_for(Some((2, 37, 7))), [Feature::MergeTree, Feature::UpdateRefs]);
        assert!(missing_for(Some((2, 38, 0))).is_empty());
        assert!(missing_for(Some((3, 0, 0))).is_empty());
        assert!(missing_for(None).is_empty(), "unknown: let git say what fails");
        assert_eq!(full_support(), (2, 38));
        assert_eq!(full_support(), crate::conflict_check::MIN_GIT);
    }

    #[test]
    fn the_problem_names_the_minimum_and_what_is_off() {
        let old = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 34, 1)), chosen: false };
        let b = old.problem().unwrap();
        assert!(b.contains("2.34.1") && b.contains("2.38 or newer") && b.contains("sparse checkout, predicting merge conflicts"), "{b}");
        assert!(!old.has(Feature::MergeTree) && old.has(Feature::Restore) && old.has(Feature::SubmoduleEdit));
        let older = GitInfo { version: Some((2, 24, 1)), ..old.clone() }.problem().unwrap();
        assert!(older.contains("these are off: editing a submodule's URL and branch, downloading file contents"), "{older}");
        assert_eq!(GitInfo { path: None, version: Some((2, 45, 0)), chosen: false }.problem(), None);
        let none = GitInfo::default().problem().unwrap();
        assert!(none.contains("couldn't find git") && none.contains("Preferences > Git"), "{none}");
        assert!(!none.contains("  "), "no runs of spaces in the text: {none}");
        let broken = GitInfo { path: Some("/x/git".into()), version: None, chosen: false }.problem().unwrap();
        assert!(broken.contains("couldn't run git") && !broken.contains("Preferences > Git"), "{broken}");
        let chosen = GitInfo { path: Some("/x/git".into()), version: None, chosen: true }.problem().unwrap();
        assert!(chosen.contains("couldn't run git") && chosen.contains("chosen in Preferences > Git"), "{chosen}");
        assert_eq!(Feature::SparseCheckout.needs_text(Some((2, 30, 1))), "Sparse checkout needs git 2.35 or newer (you have 2.30.1).");
    }

    #[test]
    fn the_banner_is_one_short_sentence() {
        let at = |v| GitInfo { path: Some("/usr/bin/git".into()), version: Some(v), chosen: false };
        assert_eq!(
            at((2, 34, 1)).banner().unwrap(),
            "Git 2.34.1 is older than 2.38, so sparse checkout, predicting merge conflicts and restacking branches are off."
        );
        assert_eq!(
            at((2, 37, 7)).banner().unwrap(),
            "Git 2.37.7 is older than 2.38, so predicting merge conflicts and restacking branches are off."
        );
        assert_eq!(at((2, 24, 1)).banner().unwrap(), "Git 2.24.1 is older than 2.38, so 6 features are off. Preferences > Git lists them.");
        assert_eq!(at((2, 45, 0)).banner(), None);
        for unusable in [
            GitInfo::default(),
            GitInfo { path: Some("/x/git".into()), version: None, chosen: false },
            GitInfo { path: Some("/x/git".into()), version: None, chosen: true },
        ] {
            assert_eq!(unusable.banner(), unusable.problem(), "{unusable:?}");
            assert!(unusable.banner().is_some());
        }
        assert_eq!(and_list(&["a"]), "a");
        assert_eq!(and_list(&["a", "b"]), "a and b");
        assert_eq!(and_list(&["a", "b", "c"]), "a, b and c");
        assert_eq!(and_list(&[]), "");
        let one = GitInfo { version: Some((2, 37, 7)), ..GitInfo::default() };
        assert!(one.missing().len() == 2 && one.banner().unwrap().ends_with("are off."));
    }

    #[test]
    fn one_missing_feature_is_off() {
        // No released git lacks exactly one feature today, so check the wording directly.
        assert_eq!(too_old_text((2, 37, 0), &["sparse checkout"]), "Git 2.37.0 is older than 2.38, so sparse checkout is off.");
    }

    #[test]
    fn download_pages_per_os() {
        use crate::opens::Os;
        assert_eq!(download_url_for(Os::Windows), "https://git-scm.com/download/win");
        assert_eq!(download_url_for(Os::MacOs), "https://git-scm.com/download/mac");
        assert_eq!(download_url_for(Os::Linux), "https://git-scm.com/download/linux");
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert!(download_url_for(os).starts_with("https://git-scm.com/download/"));
        }
        assert_eq!(download_url(), download_url_for(Os::current()));
    }

    #[test]
    fn skipping_a_sequenced_commit_needs_2_23() {
        assert_eq!(Feature::SequencerSkip.min(), (2, 23));
        assert!(Feature::ALL.contains(&Feature::SequencerSkip));
        assert!(missing_for(Some((2, 22, 0))).contains(&Feature::SequencerSkip));
        assert!(!missing_for(Some((2, 23, 0))).contains(&Feature::SequencerSkip));
        assert_eq!(
            Feature::SequencerSkip.needs_text(Some((2, 22, 0))),
            "Skipping a commit in a cherry-pick or revert needs git 2.23 or newer (you have 2.22.0)."
        );
    }

    #[test]
    fn downloading_missing_contents_needs_2_29() {
        assert_eq!(Feature::FetchObjects.min(), (2, 29));
        assert!(missing_for(Some((2, 28, 9))).contains(&Feature::FetchObjects));
        assert!(!missing_for(Some((2, 29, 0))).contains(&Feature::FetchObjects));
        let old = GitInfo { version: Some((2, 28, 0)), ..GitInfo::default() };
        assert!(!old.has(Feature::FetchObjects));
        assert_eq!(
            Feature::FetchObjects.needs_text(Some((2, 28, 0))),
            "Downloading file contents a partial clone is missing needs git 2.29 or newer (you have 2.28.0)."
        );
    }

    #[test]
    fn range_diff_needs_2_19() {
        assert_eq!(Feature::RangeDiff.min(), (2, 19));
        assert_eq!(Feature::ALL[0], Feature::RangeDiff, "the oldest minimum comes first");
        assert!(missing_for(Some((2, 18, 0))).contains(&Feature::RangeDiff));
        assert!(!missing_for(Some((2, 19, 0))).contains(&Feature::RangeDiff));
        assert_eq!(
            Feature::RangeDiff.needs_text(Some((2, 18, 0))),
            "Comparing two versions of a branch needs git 2.19 or newer (you have 2.18.0)."
        );
    }

    #[test]
    fn oldest_gated_is_the_first_feature_minimum() {
        assert_eq!(oldest_gated(), (2, 19));
        assert_eq!(oldest_gated(), Feature::RangeDiff.min());
        assert!(!new_enough((2, 18, 9)));
        assert!(new_enough((2, 19, 0)) && new_enough((2, 34, 1)));
    }

    #[test]
    fn restacking_needs_2_38() {
        assert_eq!(Feature::UpdateRefs.min(), (2, 38));
        assert!(Feature::ALL.contains(&Feature::UpdateRefs));
        assert!(missing_for(Some((2, 37, 9))).contains(&Feature::UpdateRefs));
        assert!(!missing_for(Some((2, 38, 0))).contains(&Feature::UpdateRefs));
        assert_eq!(Feature::UpdateRefs.needs_text(Some((2, 37, 1))), "Restacking branches needs git 2.38 or newer (you have 2.37.1).");
    }

    #[test]
    fn this_git_is_found_and_has_a_version() {
        let info = detect(&crate::paths::temp_dir());
        assert!(info.version.is_some(), "{info:?}");
        let path = info.path.expect("git is on PATH");
        assert!(path.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("git")), "{}", path.display());
    }
}
