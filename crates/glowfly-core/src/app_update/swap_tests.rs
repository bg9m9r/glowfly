use super::*;

fn entry(name: &str, bytes: &[u8]) -> Entry {
    Entry { name: name.into(), bytes: bytes.to_vec() }
}

/// A fresh, empty folder under the temp folder.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-swap-{tag}-{}", std::process::id()));
    let _ = persist::remove_tree(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_plan_targets_the_program_and_docs_beside_it() {
    let dir = crate::paths::temp_dir().join("glowfly-swap-plan");
    let exe = dir.join("glowfly-portable.exe");
    let entries = [entry("README.md", b"r"), entry("glowfly.exe", b"p"), entry("CHANGELOG.md", b"c")];
    let readme = dir.join("README.md");
    let plan = plan(&entries, &exe, TargetOs::Windows, |p| p == readme).unwrap();
    assert_eq!(
        plan.items,
        [SwapItem { entry: 1, target: exe.clone(), program: true }, SwapItem { entry: 0, target: readme.clone(), program: false }]
    );
    // Nothing beside it: only the program.
    assert_eq!(super::plan(&entries, &exe, TargetOs::Windows, |_| false).unwrap().items.len(), 1);
}

#[test]
fn the_plan_needs_the_program() {
    let entries = [entry("README.md", b"r")];
    let exe = crate::paths::temp_dir().join("glowfly");
    assert_eq!(plan(&entries, &exe, TargetOs::Windows, |_| true), Err("The archive has no glowfly.exe.".to_owned()));
    assert_eq!(plan(&entries, &exe, TargetOs::Linux, |_| true), Err("The archive has no glowfly.".to_owned()));
    // The Windows program isn't the Linux one.
    assert!(plan(&[entry("glowfly.exe", b"p")], &exe, TargetOs::Linux, |_| true).is_err());
}

#[test]
fn apply_swaps_and_rollback_puts_back() {
    let dir = scratch("apply");
    let exe = dir.join("old.exe");
    let readme = dir.join("README.md");
    std::fs::write(&exe, "old").unwrap();
    std::fs::write(&readme, "old readme").unwrap();
    let entries = [entry("glowfly.exe", b"new"), entry("README.md", b"new readme"), entry("LICENSE-MIT", b"mit")];
    let plan = plan(&entries, &exe, TargetOs::Windows, Path::exists).unwrap();
    let applied = apply(&plan, &entries).unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"new");
    assert_eq!(std::fs::read(&readme).unwrap(), b"new readme");
    let (exe_bak, readme_bak) = (dir.join("old.exe.bak"), dir.join("README.md.bak"));
    assert_eq!(applied.backups, [exe_bak.clone(), readme_bak.clone()]);
    assert_eq!(std::fs::read(&exe_bak).unwrap(), b"old");
    assert_eq!(std::fs::read(&readme_bak).unwrap(), b"old readme");
    assert!(!dir.join("LICENSE-MIT").exists(), "a doc file that wasn't there stays away");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&exe).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "the new program is executable: {mode:o}");
    }
    applied.rollback().unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"old");
    assert_eq!(std::fs::read(&readme).unwrap(), b"old readme");
    assert!(!exe_bak.exists() && !readme_bak.exists());
    let _ = persist::remove_tree(&dir);
}

#[cfg(unix)]
#[test]
fn a_read_only_folder_fails_and_leaves_the_program() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    let exe = dir.join("glowfly");
    std::fs::write(&exe, "old").unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores folder permissions, so the swap would go through.
    if std::fs::write(dir.join("probe"), "x").is_ok() {
        eprintln!("skipped: running as root");
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
        let _ = persist::remove_tree(&dir);
        return;
    }
    let entries = [entry("glowfly", b"new")];
    let plan = plan(&entries, &exe, TargetOs::Linux, Path::exists).unwrap();
    let err = apply(&plan, &entries).unwrap_err();
    assert!(err.starts_with("Couldn't replace Glowfly's program in "), "{err}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"old");
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
    let _ = persist::remove_tree(&dir);
}
