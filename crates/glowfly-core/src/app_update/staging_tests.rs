//! The staging folder: fresh and private, its names, tidying left-over ones, and falling back
//! to another place when the helper can't run from the temporary folder.

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

/// Three places: a temp folder, the program's folder and a cache folder, with a program.
fn places(tag: &str) -> (PathBuf, Vec<PathBuf>, PathBuf) {
    let root = crate::paths::temp_dir().join(format!("glowfly-staging-{tag}-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&root);
    let places: Vec<PathBuf> = ["temp", "app", "cache"].iter().map(|n| root.join(n)).collect();
    for p in &places {
        std::fs::create_dir_all(p).unwrap();
    }
    let exe = places[1].join("glowfly");
    std::fs::write(&exe, "program").unwrap();
    (root, places, exe)
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect()
}

#[test]
fn the_helper_is_staged_in_the_first_place_it_runs() {
    let (root, places, exe) = places("first");
    let s = create_runnable(&places, &exe, "glowfly-update", &|_| Ok(())).unwrap();
    assert_eq!(s.dir.parent(), Some(places[0].as_path()));
    assert_eq!((s.helper.clone(), std::fs::read(&s.helper).unwrap()), (s.dir.join("glowfly-update"), b"program".to_vec()));
    let _ = crate::persist::remove_tree(&root);
}

#[test]
fn a_noexec_temp_folder_falls_back_beside_the_program_then_to_the_cache() {
    let (root, places, exe) = places("noexec");
    let refused = |dirs: Vec<PathBuf>| {
        move |helper: &Path| {
            if dirs.iter().any(|d| helper.starts_with(d)) {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else {
                Ok(())
            }
        }
    };
    let s = create_runnable(&places, &exe, "glowfly-update", &refused(vec![places[0].clone()])).unwrap();
    assert_eq!(s.dir.parent(), Some(places[1].as_path()));
    assert!(entries(&places[0]).is_empty(), "the refused folder is removed");
    let _ = crate::persist::remove_tree(&s.dir);
    let s = create_runnable(&places, &exe, "glowfly-update", &refused(places[..2].to_vec())).unwrap();
    assert_eq!(s.dir.parent(), Some(places[2].as_path()));
    assert_eq!(entries(&places[1]), vec![exe.clone()]);
    let _ = crate::persist::remove_tree(&s.dir);
    // Nowhere: each place's reason is said, and nothing is left.
    let err = create_runnable(&places, &exe, "glowfly-update", &refused(places.clone())).unwrap_err();
    assert!(err.starts_with("Couldn't find a folder the update helper can run from ("), "{err}");
    assert_eq!(err.matches(": ").count(), 3, "{err}");
    assert!(entries(&places[0]).is_empty() && entries(&places[2]).is_empty());
    // A place that doesn't exist (or is named twice) is passed over.
    let more = vec![root.join("missing"), places[2].clone(), places[2].clone()];
    let s = create_runnable(&more, &exe, "glowfly-update", &|_| Ok(())).unwrap();
    assert_eq!(s.dir.parent(), Some(places[2].as_path()));
    let _ = crate::persist::remove_tree(&root);
}
