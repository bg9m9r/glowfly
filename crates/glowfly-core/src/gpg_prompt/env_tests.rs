use super::env::{allows_prompts, vars};
use std::ffi::OsStr;
use std::path::Path;

#[test]
fn points_git_at_trellis_unless_config_was_inherited() {
    let exe = if cfg!(windows) { Path::new(r"C:\Apps\Trellis\glowfly.exe") } else { Path::new("/opt/trellis/bin/trellis") };
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
    let v = vars(Path::new(r"\\?\C:\Apps/Trellis/glowfly.exe"), None, OsStr::new("git"));
    let value = v.iter().find(|(n, _)| *n == "GIT_CONFIG_VALUE_0").map(|(_, x)| x.clone()).unwrap();
    assert_eq!(value, OsStr::new(r"C:\Apps\Trellis\glowfly.exe"));
}

#[test]
fn no_prompts_leaves_gpg_alone() {
    assert!(!allows_prompts(crate::gitcli::NO_PROMPTS));
    assert!(allows_prompts(&[]));
    assert!(allows_prompts(&[("GIT_SEQUENCE_EDITOR", "x")]));
}

#[test]
fn each_command_gets_the_signing_env_only_when_it_may_prompt() {
    use super::env::for_command;
    let exe = if cfg!(windows) { Path::new(r"C:\Apps\Trellis\glowfly.exe") } else { Path::new("/opt/trellis/bin/trellis") };
    let a = crate::gitcli::AskpassEnv { exe: exe.to_owned(), addr: "127.0.0.1:1".into(), token: "t".into() };
    let cwd = crate::paths::temp_dir();
    let status = crate::gitcli::args(&["status"]);
    assert!(for_command(&cwd, &status, &[], None).is_empty(), "before the helper is enabled");
    assert!(for_command(&cwd, &status, crate::gitcli::NO_PROMPTS, Some(&a)).is_empty());
    let commit = crate::gitcli::args(&["commit", "-m", "x"]);
    assert!(for_command(&cwd, &commit, crate::gitcli::NO_PROMPTS, Some(&a)).is_empty());
    let v = for_command(&cwd, &status, &[], Some(&a));
    let inherited = std::env::var_os("GIT_CONFIG_COUNT");
    assert_eq!(v, vars(exe, inherited.as_deref(), &crate::git_program::program()));
    assert!(v.iter().all(|(k, _)| *k != crate::askpass::ENV_SSH_SIGNING_KEY));
}
