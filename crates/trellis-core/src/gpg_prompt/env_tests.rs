use super::env::{allows_prompts, vars};
use std::ffi::OsStr;
use std::path::Path;

#[test]
fn points_git_at_trellis_unless_config_was_inherited() {
    let exe = if cfg!(windows) { Path::new(r"C:\Apps\Trellis\trellis.exe") } else { Path::new("/opt/trellis/bin/trellis") };
    for inherited in [None, Some(OsStr::new(""))] {
        let v = vars(exe, inherited, OsStr::new("/x/git"));
        let get = |k: &str| v.iter().find(|(n, _)| *n == k).map(|(_, x)| x.clone());
        assert_eq!(v.len(), 5);
        assert_eq!(get("GIT_CONFIG_COUNT").as_deref(), Some(OsStr::new("1")));
        assert_eq!(get("GIT_CONFIG_KEY_0").as_deref(), Some(OsStr::new("gpg.program")));
        assert_eq!(get("GIT_CONFIG_VALUE_0").as_deref(), Some(exe.as_os_str()));
        assert_eq!(get("TRELLIS_GPG_WRAPPER").as_deref(), Some(OsStr::new("1")));
        assert_eq!(get(super::ENV_GIT).as_deref(), Some(OsStr::new("/x/git")), "the wrapper asks the same git");
    }
    assert!(vars(exe, Some(OsStr::new("2")), OsStr::new("/x/git")).is_empty(), "the user's own command-scope config is kept");
}

#[test]
#[cfg(windows)]
fn the_exe_is_given_in_windows_spelling() {
    let v = vars(Path::new(r"\\?\C:\Apps/Trellis/trellis.exe"), None, OsStr::new("git"));
    let value = v.iter().find(|(n, _)| *n == "GIT_CONFIG_VALUE_0").map(|(_, x)| x.clone()).unwrap();
    assert_eq!(value, OsStr::new(r"C:\Apps\Trellis\trellis.exe"));
}

#[test]
fn no_prompts_leaves_gpg_alone() {
    assert!(!allows_prompts(crate::gitcli::NO_PROMPTS));
    assert!(allows_prompts(&[]));
    assert!(allows_prompts(&[("GIT_SEQUENCE_EDITOR", "x")]));
}
