//! git-lfs's version: the text `git lfs version` prints, its numbers, and the oldest git-lfs the
//! commands Glowfly runs work with.

/// Major, minor and patch.
pub type Version = (u32, u32, u32);

/// The oldest git-lfs Glowfly works with. The newest requirement among the commands it runs is
/// `git lfs track --filename` (2.9.0), used by Track in Git LFS > Only this file. The filter
/// protocol the installed config uses came with 1.5.0 and the escaping of `#` and spaces in
/// patterns with 2.0.2, both older.
pub const MIN: Version = (2, 9, 0);

/// The version in `git lfs version` output (`git-lfs/3.7.1 (GitHub; ...)` gives `3.7.1`).
pub fn parse_version(out: &str) -> Option<String> {
    let rest = out.lines().next()?.trim().strip_prefix("git-lfs/")?;
    let version = rest.split(char::is_whitespace).next()?;
    (!version.is_empty()).then(|| version.to_owned())
}

/// The numbers of a version: the leading digits of its first three dot-separated parts, so
/// `2.3.0-pre` and `3.0.2-1ubuntu1` read as (2, 3, 0) and (3, 0, 2). A missing patch is 0; None
/// when major or minor has no number.
pub fn numbers(version: &str) -> Option<Version> {
    let mut parts = version.split('.').map(|p| {
        let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
        (!digits.is_empty()).then(|| digits.parse::<u32>().ok()).flatten()
    });
    let major = parts.next()??;
    let minor = parts.next()??;
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

/// Whether `version` is known to be older than [`MIN`]. A version that can't be read as numbers
/// is never too old (the same rule as an unreadable git version).
pub fn too_old(version: &str) -> bool {
    numbers(version).is_some_and(|v| v < MIN)
}

/// [`MIN`] as text ("2.9.0").
pub fn min_text() -> String {
    crate::git_version::show(MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_text_in_its_known_forms() {
        let v = |s: &str| parse_version(s);
        assert_eq!(v("git-lfs/3.7.1 (GitHub; windows amd64; go 1.25.1; git b84b3384)"), Some("3.7.1".into()));
        assert_eq!(v("git-lfs/3.4.1 (GitHub; linux amd64; go 1.21.3)\n"), Some("3.4.1".into()));
        assert_eq!(v("git-lfs/2.13.2 (Homebrew; darwin arm64; go 1.16)\r\n"), Some("2.13.2".into()));
        assert_eq!(v("git-lfs/2.3.0-pre (GitHub; linux amd64; go 1.9)"), Some("2.3.0-pre".into()));
        assert_eq!(v("git-lfs/3.7.1"), Some("3.7.1".into()));
        assert_eq!(v("git: 'lfs' is not a git command. See 'git --help'."), None);
        assert_eq!(v(""), None);
        assert_eq!(v("git-lfs/"), None);
        assert_eq!(v("git-lfs/ (GitHub; linux amd64)"), None);
    }

    #[test]
    fn numbers_of_a_version() {
        assert_eq!(numbers("3.7.1"), Some((3, 7, 1)));
        assert_eq!(numbers("2.9"), Some((2, 9, 0)));
        assert_eq!(numbers("2.3.0-pre"), Some((2, 3, 0)));
        assert_eq!(numbers("3.0.0-rc1"), Some((3, 0, 0)));
        assert_eq!(numbers("2.13.2+dfsg"), Some((2, 13, 2)));
        assert_eq!(numbers("3.0.2-1ubuntu1"), Some((3, 0, 2)));
        assert_eq!(numbers("2.9.0.windows.1"), Some((2, 9, 0)));
        assert_eq!(numbers("3.7."), Some((3, 7, 0)));
        for bad in ["", "main", "3", "3.x", "x.7.1", "v3.7.1", "99999999999.1.0", "(GitHub;"] {
            assert_eq!(numbers(bad), None, "{bad}");
        }
    }

    #[test]
    fn too_old_only_when_known_to_be_older() {
        for old in ["2.8.9", "2.3.4", "1.5.6", "0.6.0"] {
            assert!(too_old(old), "{old}");
        }
        for fine in ["2.9.0", "2.9.0-pre", "2.10.0", "3.7.1", "main", ""] {
            assert!(!too_old(fine), "{fine}");
        }
        assert_eq!(MIN, (2, 9, 0));
        assert_eq!(min_text(), "2.9.0");
    }

    #[test]
    fn the_installed_git_lfs_reads_as_numbers() {
        let Some(v) = crate::testutil::lfs_or_skip("the_installed_git_lfs_reads_as_numbers") else { return };
        assert!(numbers(&v).is_some(), "{v}");
    }
}
