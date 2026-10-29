use super::*;

#[test]
fn reads_openssh_versions() {
    assert_eq!(parse("OpenSSH_8.2p1 Ubuntu-4ubuntu0.5, OpenSSL 1.1.1f  31 Mar 2020"), Some((8, 2)));
    assert_eq!(parse("OpenSSH_9.6p1, LibreSSL 3.3.6"), Some((9, 6)));
    assert_eq!(parse("OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2"), Some((9, 5)));
    assert_eq!(parse("OpenSSH_10.0p2"), Some((10, 0)));
    assert_eq!(parse("Dropbear v2022.83"), None);
    assert_eq!(parse(""), None);
}

#[test]
fn versions_before_8_4_need_the_legacy_set_up() {
    assert!(needs_legacy(Some((7, 4))));
    assert!(needs_legacy(Some((8, 3))));
    assert!(!needs_legacy(Some((8, 4))));
    assert!(!needs_legacy(Some((10, 0))));
    assert!(needs_legacy(None), "an unknown version keeps the safe set-up");
}

#[test]
fn picks_the_ssh_git_runs_in_gits_order() {
    fn os(s: &str) -> Option<&OsStr> {
        Some(OsStr::new(s))
    }
    let shell = |s: &str| SshCommand::Shell(s.into());
    let program = |s: &str| SshCommand::Program(s.into());
    assert_eq!(ssh_command(os("ssh -i key"), os("ssh -v"), os("plink")), shell("ssh -i key"), "GIT_SSH_COMMAND wins");
    assert_eq!(
        ssh_command(None, os("'/my ssh/ssh' -v"), os("plink")),
        shell("'/my ssh/ssh' -v"),
        "then core.sshCommand, kept whole for sh"
    );
    assert_eq!(ssh_command(os(""), os("ssh -v"), None), shell(""), "a set GIT_SSH_COMMAND counts even when empty, as in git");
    assert_eq!(ssh_command(None, None, os("/usr/local/bin/ssh")), program("/usr/local/bin/ssh"), "then GIT_SSH, run directly");
    assert_eq!(ssh_command(None, None, os("")), program("ssh"));
    assert_eq!(ssh_command(None, None, None), program("ssh"));
}

#[test]
fn reads_the_repositorys_core_ssh_command() {
    let dir = crate::testutil::scratch_repo("ssh-version-config");
    crate::testkit::git(&dir, &["config", "core.sshCommand", "'/opt/my ssh/ssh' -i 'a key'"]);
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    assert_eq!(
        configured_ssh_command(Some(&sub)).as_deref(),
        Some(OsStr::new("'/opt/my ssh/ssh' -i 'a key'")),
        "from a folder inside it too"
    );
    assert_eq!(ssh_command_in(Some(&dir), None, Some(OsStr::new("plink"))), SshCommand::Shell("'/opt/my ssh/ssh' -i 'a key'".into()));
    let env = [("GIT_SSH_COMMAND", "ssh -v")];
    assert_eq!(ssh_command_for(Some(&dir), &env), SshCommand::Shell("ssh -v".into()), "the environment overrides the config");
    assert!(
        needs_legacy(None) && (!cfg!(unix) || legacy_for(&ssh_command_in(Some(&dir), None, None))),
        "a command line gets the safe set-up"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn every_prepared_child_leads_its_own_session() {
    // Whatever ssh git runs: the session no longer depends on its version.
    let mut cmd = Command::new("sleep");
    cmd.arg("5");
    prepare(&mut cmd);
    let mut child = cmd.spawn().expect("sleep starts");
    let pid = child.id() as libc::pid_t;
    // SAFETY: getsid only reads the session id of the given process.
    let session = unsafe { libc::getsid(pid) };
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(session, pid);
}

/// An executable `/bin/sh` script running `body`, in a folder whose name has a space: a fake old
/// OpenSSH, or one that only leaves a file behind when it runs.
#[cfg(unix)]
fn script(name: &str, body: &str) -> (std::path::PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::paths::temp_dir().join(format!("glowfly-{name}-{}", std::process::id())).join("my ssh");
    std::fs::create_dir_all(&dir).unwrap();
    let ssh = dir.join("old ssh");
    std::fs::write(&ssh, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = ssh.to_str().unwrap().to_owned();
    (dir, path)
}

#[cfg(unix)]
#[test]
fn reads_an_environment_programs_version() {
    let (dir, ssh) = script("ssh-version-program", "[ \"$1\" = -V ] && echo 'OpenSSH_7.9p1, OpenSSL 1.1.1' >&2");
    assert_eq!(detect(OsStr::new(&ssh)), Some((7, 9)));
    assert!(legacy_for(&SshCommand::Program(ssh.into())), "an old GIT_SSH gets the legacy set-up");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn a_repositorys_ssh_command_never_runs() {
    let (dir, ssh) = script("ssh-version-marker", "touch \"$(dirname \"$0\")/ran\"");
    let marker = dir.join("ran");
    let repo = crate::testutil::scratch_repo("ssh-version-marker");
    crate::testkit::git(&repo, &["config", "core.sshCommand", &format!("'{ssh}'")]);
    assert!(legacy_askpass_in(Some(&repo), &[]), "the safe set-up, without reading its version");
    assert!(legacy_for(&ssh_command_in(Some(&repo), None, None)));
    let a = crate::gitcli::AskpassEnv { exe: "glowfly".into(), addr: "127.0.0.1:1".into(), token: "ab".into() };
    let _ = crate::gitcli::askpass_vars_in(&a, Some(&repo), &[]);
    let read = crate::gitcli::output(&repo, &["config".to_owned(), "--get".to_owned(), "core.sshCommand".to_owned()], "", &[], None);
    assert!(read.ok, "{read:?}");
    assert!(!marker.exists(), "nothing a repository's config names runs for a read");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let _ = std::fs::remove_dir_all(&repo);
}

#[cfg(unix)]
#[test]
fn a_detached_child_leads_its_own_session() {
    let mut cmd = Command::new("sleep");
    cmd.arg("5");
    detach_from_terminal(&mut cmd);
    let mut child = cmd.spawn().expect("sleep starts");
    let pid = child.id() as libc::pid_t;
    // SAFETY: getsid only reads the session id of the given process.
    let session = unsafe { libc::getsid(pid) };
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(session, pid, "the child started a new session");
}
