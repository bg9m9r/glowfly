//! The private folder an update is prepared in: `<temp>/glowfly-update-<16 hex>`, made fresh
//! (never reused, so an existing name or a planted link fails) and, on Unix, readable by this
//! user only. It holds the helper (a copy of the running program), its plan and an MSI's copy.
//! On Windows the helper can't delete its own program, so [`tidy`] removes old folders at the
//! next start.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const PREFIX: &str = "glowfly-update-";
/// A staging folder older than this is left over.
pub const STALE_AFTER: Duration = Duration::from_secs(3600);

/// Creates a new staging folder inside `temp`.
pub fn create(temp: &Path) -> std::io::Result<PathBuf> {
    let mut last = None;
    for _ in 0..3 {
        let mut raw = [0u8; 8];
        getrandom::fill(&mut raw).map_err(|e| std::io::Error::other(e.to_string()))?;
        let name: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let dir = temp.join(format!("{PREFIX}{name}"));
        match make_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free staging folder name")))
}

/// Creates `dir` itself (not its parents): mode 0700 on Unix.
fn make_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::DirBuilder::new().create(dir)
    }
}

/// Whether `name` is one [`create`] makes: the prefix and 16 lower-case hex digits.
pub fn is_staging_name(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|rest| rest.len() == 16 && rest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Whether a folder last changed at `modified` is left over at `now`; one from the future
/// isn't.
pub fn stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > STALE_AFTER)
}

/// Removes the left-over staging folders in `temp` (real folders, not links, named as
/// [`create`] names them and older than [`STALE_AFTER`]); errors are ignored. Returns how many
/// were removed.
pub fn tidy(temp: &Path, now: SystemTime) -> usize {
    let Ok(list) = std::fs::read_dir(temp) else { return 0 };
    let mut removed = 0;
    for entry in list.flatten() {
        if !is_staging_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let old = meta.modified().is_ok_and(|m| stale(m, now));
        if meta.is_dir() && !meta.file_type().is_symlink() && old && crate::persist::remove_tree(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_makes_a_fresh_private_folder() {
        let temp = crate::paths::temp_dir().join(format!("glowfly-staging-create-{}", std::process::id()));
        let _ = crate::persist::remove_tree(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let a = create(&temp).unwrap();
        let b = create(&temp).unwrap();
        assert_ne!(a, b);
        for d in [&a, &b] {
            assert_eq!(d.parent(), Some(temp.as_path()));
            assert!(is_staging_name(&d.file_name().unwrap().to_string_lossy()), "{}", d.display());
            assert!(d.is_dir());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&a).unwrap().permissions().mode() & 0o777, 0o700);
        }
        // Fresh: nothing of an existing folder is reused, and the temp folder must exist.
        assert!(create(&temp.join("missing")).is_err());
        let _ = crate::persist::remove_tree(&temp);
    }

    #[test]
    fn staging_names() {
        assert!(is_staging_name("glowfly-update-0123456789abcdef"));
        for name in [
            "glowfly-update-XYZ",
            "glowfly-update-0123456789ABCDEF",
            "glowfly-update-0123456789abcde",
            "glowfly-update-0123456789abcdef0",
            "glowfly-update-",
            "other-0123456789abcdef",
        ] {
            assert!(!is_staging_name(name), "{name}");
        }
    }

    #[test]
    fn stale_after_an_hour() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let ago = |mins: u64| now - Duration::from_secs(mins * 60);
        assert!(!stale(ago(59), now));
        assert!(stale(ago(61), now));
        assert!(!stale(now + Duration::from_secs(600), now), "from the future");
    }

    #[test]
    fn tidy_keeps_fresh_and_unrelated_entries() {
        let temp = crate::paths::temp_dir().join(format!("glowfly-staging-tidy-{}", std::process::id()));
        let _ = crate::persist::remove_tree(&temp);
        std::fs::create_dir_all(temp.join("unrelated")).unwrap();
        std::fs::write(temp.join("glowfly-update-aaaaaaaaaaaaaaaa"), "a file").unwrap();
        let fresh = create(&temp).unwrap();
        assert_eq!(tidy(&temp, SystemTime::now()), 0);
        assert!(fresh.is_dir() && temp.join("unrelated").is_dir() && temp.join("glowfly-update-aaaaaaaaaaaaaaaa").is_file());
        // Two hours on, the staging folder is left over; the file and the other folder stay.
        assert_eq!(tidy(&temp, SystemTime::now() + Duration::from_secs(7200)), 1);
        assert!(!fresh.exists() && temp.join("unrelated").is_dir() && temp.join("glowfly-update-aaaaaaaaaaaaaaaa").is_file());
        let _ = crate::persist::remove_tree(&temp);
    }
}
