//! Why the trash refused a file: the platform codes and settings that mean "no trash", "in use"
//! and "access denied", as table tests that run on every system, plus Windows checks against
//! the real system.

use super::*;
use TrashProblem::*;

#[test]
fn hresults_from_the_shell() {
    for (hr, want) in [
        (0x8027_0027_u32, Some(InUse)),
        (0x8027_0028, Some(InUse)),
        (0x8007_0020, Some(InUse)),
        (0x8007_0021, Some(InUse)),
        (0x8027_0021, Some(AccessDenied)),
        (0x8027_0022, Some(AccessDenied)),
        (0x8027_0002, Some(AccessDenied)),
        (0x8007_0005, Some(AccessDenied)),
        (0x8007_0013, Some(AccessDenied)),
        (0x8027_0036, Some(NoTrash)),
        (0x8027_0037, Some(NoTrash)),
        (0x8027_0038, Some(NoTrash)),
        (0x8027_003A, Some(NoTrash)),
        // Unknown copy-engine and Windows errors say nothing.
        (0x8027_0035, None),
        (0x8007_0002, None),
        (0x8000_4005, None),
    ] {
        assert_eq!(problem_of_hresult(hr as i32), want, "{hr:#x}");
    }
}

#[test]
fn windows_error_codes() {
    assert_eq!([32, 33, 5, 19, 2].map(problem_of_win32), [Some(InUse), Some(InUse), Some(AccessDenied), Some(AccessDenied), None]);
}

#[test]
fn io_errors_from_the_freedesktop_trash() {
    use std::io::ErrorKind as K;
    for (kind, want) in [
        (K::PermissionDenied, Some(AccessDenied)),
        (K::ReadOnlyFilesystem, Some(AccessDenied)),
        (K::ResourceBusy, Some(InUse)),
        (K::ExecutableFileBusy, Some(InUse)),
        (K::NotFound, None),
        (K::CrossesDevices, None),
        (K::StorageFull, None),
    ] {
        assert_eq!(problem_of_io(kind), want, "{kind:?}");
    }
}

#[test]
fn a_home_trash_needs_xdg_data_home_or_home() {
    let env = |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).into());
    assert!(no_home_trash(env(&[])));
    assert!(no_home_trash(env(&[("HOME", ""), ("XDG_DATA_HOME", "")])));
    assert!(!no_home_trash(env(&[("HOME", "/home/ada")])));
    assert!(!no_home_trash(env(&[("XDG_DATA_HOME", "/data")])));
}

#[test]
fn volume_flags_on_macos() {
    assert_eq!(problem_of_volume(true, false), None, "a local volume has a Trash: the reason is elsewhere");
    assert_eq!(problem_of_volume(false, false), Some(NoTrash), "network volumes have none");
    assert_eq!(problem_of_volume(true, true), Some(AccessDenied));
    assert_eq!(problem_of_volume(false, true), Some(AccessDenied));
}

#[test]
fn recycle_bin_settings_that_would_destroy_a_file() {
    let mb = 1024 * 1024;
    let s = |nuke, max: Option<u64>, policy| BinSettings { nuke_on_delete: nuke, max_capacity_mb: max, policy_off: policy };
    for (settings, size, refused) in [
        (BinSettings::default(), 10 * mb, None),
        (s(false, Some(100), false), 100 * mb, None),
        (s(false, Some(100), false), 100 * mb + 1, Some("larger than this drive's Recycle Bin (100 MB)")),
        (s(true, Some(100), false), 1, Some("turned off for this drive")),
        (s(false, None, true), 1, Some("policy")),
        (s(false, Some(0), false), 1, Some("(0 MB)")),
        (s(false, Some(0), false), 0, None),
    ] {
        let got = bin_refuses(&settings, size);
        match refused {
            None => assert_eq!(got, None, "{settings:?} {size}"),
            Some(part) => assert!(got.as_deref().is_some_and(|g| g.contains(part)), "{settings:?} {size}: {got:?}"),
        }
    }
}

#[cfg(windows)]
#[test]
fn the_system_drive_has_readable_bin_settings() {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let root = std::ffi::OsString::from(format!("{drive}\\"));
    assert!(win::has_recycle_bin(&root).is_ok(), "{drive} has a Recycle Bin");
    // Reading never fails; on a default system nothing in it refuses an ordinary file.
    let settings = win::bin_settings(&root);
    if settings == BinSettings::default() {
        assert_eq!(bin_refuses(&settings, 1024), None);
    }
}

#[cfg(windows)]
#[test]
fn a_file_another_program_holds_is_in_use() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = crate::paths::temp_dir().join(format!("glowfly-trash-in-use-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("held.txt");
    std::fs::write(&file, "x").unwrap();
    let refusal = || win::refusal(&file, trash::Error::Unknown { description: "Some operations were aborted".into() });
    assert_eq!(refusal().problem, Other, "nothing holds it yet");
    // Open with no sharing, as an editor that locks its file does.
    let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&file).unwrap();
    let r = refusal();
    assert_eq!((r.problem, r.detail.as_str()), (InUse, "Some operations were aborted"));
    drop(held);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn the_mount_point_of_a_path_is_its_volume_root() {
    let dir = crate::paths::temp_dir();
    let point = win::volume_mount_point(&dir.join("some-file.txt")).expect("the temp folder is on a volume");
    let text = point.to_string_lossy().to_lowercase();
    assert!(text.ends_with('\\'), "{text}");
    assert!(dir.to_string_lossy().to_lowercase().starts_with(&text), "{text} is a prefix of {}", dir.display());
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let on_root = std::path::PathBuf::from(format!(r"{drive}\some-file.txt"));
    assert_eq!(win::volume_mount_point(&on_root), volume_root_of(&on_root));
}
