//! Which ssh git runs on Windows, decided from facts (on every OS), Trellis's value for Windows'
//! ssh, and the facts read from a fixture folder tree.

use super::tests::temp;
use super::*;
use crate::paths::same_spelling;
use std::path::{Path, PathBuf};

const WIN: &str = "C:/Windows/System32/OpenSSH/ssh.exe";
const BUNDLED: &str = "C:/Program Files/Git/usr/bin/ssh.exe";

#[test]
fn recognises_the_windows_ssh_spelling() {
    assert!(is_windows_ssh_word(WIN));
    assert!(is_windows_ssh_word(r"D:\WinNT\System32\OpenSSH\ssh.exe"));
    for other in ["ssh", "/usr/bin/ssh", "C:/Windows/system32/openssh/ssh.exe", BUNDLED, "System32/OpenSSH/ssh.exe", "C:ssh.exe"] {
        assert!(!is_windows_ssh_word(other), "{other}");
    }
}

#[test]
fn the_written_value_reads_back() {
    let value = windows_ssh_command_for(Path::new(WIN)).unwrap();
    assert_eq!(value, format!("'{WIN}'"));
    let KeyChoice::WindowsSsh(back) = parse_ssh_command(&value) else { panic!("{value}: not the Windows ssh form") };
    assert!(same_spelling(&back, Path::new(WIN)), "{back:?}");
    assert_eq!(parse_ssh_command(WIN), KeyChoice::WindowsSsh(WIN.into()), "unquoted too");
    assert!(matches!(parse_ssh_command(&format!("'{WIN}' -v")), KeyChoice::Other { .. }), "with arguments it is someone else's");
    assert!(matches!(parse_ssh_command("'C:/Windows/system32/openssh/ssh.exe'"), KeyChoice::Other { .. }));
    assert_eq!(program_word(&format!("'{WIN}' -v")), Some(WIN.into()));
    assert_eq!(program_word("ssh $X"), None);
}

fn facts() -> WinSshFacts {
    WinSshFacts { windows: Some(WIN.into()), bundled: Some(BUNDLED.into()), ..Default::default() }
}

#[test]
fn which_ssh_git_runs() {
    let key = KeyChoice::Key("C:/Users/ada/.ssh/id".into());
    let other = |v: &str| KeyChoice::Other { value: v.into(), key: None };
    let unset = KeyChoice::Unset;
    let no_bundled = WinSshFacts { bundled: None, ..facts() };
    assert_eq!(git_runs(true, &unset, &facts()), GitSsh::EnvCommand);
    assert_eq!(git_runs(false, &unset, &facts()), GitSsh::Bundled);
    let windows_on_path = WinSshFacts { on_path: Some(WIN.into()), ..no_bundled.clone() };
    assert_eq!(git_runs(false, &unset, &windows_on_path), GitSsh::WindowsOwn);
    let plink_on_path = WinSshFacts { on_path: Some("C:/tools/ssh.exe".into()), ..no_bundled.clone() };
    assert_eq!(git_runs(false, &unset, &plink_on_path), GitSsh::Other(PathBuf::from("C:/tools/ssh.exe").display().to_string()));
    assert_eq!(git_runs(false, &unset, &no_bundled), GitSsh::Unknown, "no ssh found");
    let system = WinSshFacts { system_command: Some(format!("'{WIN}'")), ..facts() };
    assert_eq!(git_runs(false, &unset, &system), GitSsh::WindowsOwn, "the system config's value");
    let git_ssh = WinSshFacts { git_ssh: Some(WIN.into()), ..facts() };
    assert_eq!(git_runs(false, &unset, &git_ssh), GitSsh::WindowsOwn, "GIT_SSH");
    let plink = WinSshFacts { git_ssh: Some("plink.exe".into()), ..facts() };
    assert_eq!(git_runs(false, &unset, &plink), GitSsh::Other("plink.exe".into()));
    assert_eq!(git_runs(false, &key, &facts()), GitSsh::Bundled, "Trellis's key form runs plain ssh");
    assert_eq!(git_runs(false, &KeyChoice::WindowsSsh(WIN.into()), &facts()), GitSsh::WindowsOwn);
    assert_eq!(git_runs(false, &other("ssh -v"), &facts()), GitSsh::Bundled);
    assert_eq!(git_runs(false, &other("plink -batch"), &facts()), GitSsh::Other("plink".into()));
    assert_eq!(git_runs(false, &other("ssh $X"), &facts()), GitSsh::Unknown);
    assert_eq!(git_runs(false, &other(&format!("\"{BUNDLED}\" -v")), &facts()), GitSsh::Bundled, "named by its path");
    let both = WinSshFacts { system_command: Some(format!("'{WIN}'")), git_ssh: Some(WIN.into()), ..facts() };
    assert_eq!(git_runs(false, &key, &both), GitSsh::Bundled, "the global value beats the system one and GIT_SSH");
}

#[test]
fn facts_from_a_folder_tree() {
    let root = temp("win-facts");
    let make = |p: &Path| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "exe").unwrap();
    };
    let windows_dir = root.join("Windows");
    let exec = root.join("Git").join("mingw64").join("libexec").join("git-core");
    let path_dir = root.join("bin");
    let path_var = std::env::join_paths([root.join("empty"), path_dir.clone()]).unwrap();

    let none = facts_from(&windows_dir, Some(&exec), Some(&path_var), None, None, |_| panic!("no ssh to ask"));
    assert_eq!(none, WinSshFacts::default());

    let windows_ssh = windows_dir.join("System32").join("OpenSSH").join("ssh.exe");
    let bundled = root.join("Git").join("usr").join("bin").join("ssh.exe");
    make(&windows_ssh);
    make(&bundled);
    make(&path_dir.join("ssh.exe"));
    let got = facts_from(&windows_dir, Some(&exec), Some(&path_var), Some("plink".into()), Some("ssh -v".into()), |p| {
        assert_eq!(p, windows_ssh.as_path());
        Some((9, 5))
    });
    assert_eq!(got.windows.as_deref(), Some(windows_ssh.as_path()));
    assert_eq!(got.windows_version, Some((9, 5)));
    assert_eq!(got.bundled.as_deref(), Some(bundled.as_path()));
    assert_eq!(got.on_path, Some(path_dir.join("ssh.exe")));
    assert_eq!((got.git_ssh.as_deref(), got.system_command.as_deref()), (Some("plink"), Some("ssh -v")));
    let no_git = facts_from(&windows_dir, None, None, None, None, |_| None);
    assert_eq!((no_git.bundled, no_git.on_path, no_git.windows_version), (None, None, None));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn writes_the_windows_ssh_value_globally() {
    let dir = temp("win-write");
    let global = dir.join("global.gitconfig");
    let global_s = global.to_string_lossy().into_owned();
    let env = [("GIT_CONFIG_GLOBAL", global_s.as_str()), ("GIT_CONFIG_NOSYSTEM", "1")];
    write_windows_ssh(&dir, Path::new(WIN), &env).unwrap();
    assert_eq!(read_choice(&dir, true, &env), Ok(KeyChoice::WindowsSsh(WIN.into())));
    write_choice(&dir, true, None, &env).unwrap();
    assert_eq!(read_choice(&dir, true, &env), Ok(KeyChoice::Unset));
    let _ = std::fs::remove_dir_all(&dir);
}
