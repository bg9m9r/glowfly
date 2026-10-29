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
    assert_eq!(ssh_command(os("  "), os("ssh -v"), None), shell("ssh -v"), "an empty command is skipped");
    assert_eq!(ssh_command(None, os(""), os("/usr/local/bin/ssh")), program("/usr/local/bin/ssh"), "then GIT_SSH, run directly");
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
    let unset_env = [("GIT_SSH_COMMAND", "")];
    assert_eq!(ssh_command_for(Some(&dir), &unset_env), SshCommand::Shell("'/opt/my ssh/ssh' -i 'a key'".into()));
    let env = [("GIT_SSH_COMMAND", "ssh -v")];
    assert_eq!(ssh_command_for(Some(&dir), &env), SshCommand::Shell("ssh -v".into()), "the environment overrides the config");
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

/// A fake old OpenSSH in a folder whose name has a space, printing its version as ssh does.
#[cfg(unix)]
fn fake_old_ssh(name: &str) -> (std::path::PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::paths::temp_dir().join(format!("glowfly-{name}-{}", std::process::id())).join("my ssh");
    std::fs::create_dir_all(&dir).unwrap();
    let ssh = dir.join("old ssh");
    std::fs::write(
        &ssh,
        "#!/bin/sh
for a; do [ \"$a\" = -V ] && echo 'OpenSSH_7.9p1, OpenSSL 1.1.1' >&2 && exit 0; done
exit 2
",
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = ssh.to_str().unwrap().to_owned();
    (dir, path)
}

#[cfg(unix)]
#[test]
fn reads_the_version_through_the_shell_as_git_runs_it() {
    let (dir, ssh) = fake_old_ssh("ssh-version-shell");
    let quoted = SshCommand::Shell(format!("'{ssh}' -o BatchMode=yes -i \"$HOME/.ssh/id\"").into());
    assert_eq!(detect(&quoted), Some((7, 9)), "quoting and variables are the shell's");
    assert!(legacy_for(&quoted));
    let wrapped = SshCommand::Shell(format!("FOO=1 \"{ssh}\"").into());
    assert_eq!(detect(&wrapped), Some((7, 9)), "an assignment before the program works as in sh");
    assert_eq!(detect(&SshCommand::Program(ssh.clone().into())), Some((7, 9)), "GIT_SSH is run directly");
    assert_eq!(detect(&SshCommand::Shell(ssh.into())), None, "unquoted, the space splits the path, as it does for git");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn a_repositorys_old_ssh_gets_the_legacy_set_up() {
    let (dir, ssh) = fake_old_ssh("ssh-version-repo");
    let repo = crate::testutil::scratch_repo("ssh-version-repo");
    crate::testkit::git(&repo, &["config", "core.sshCommand", &format!("'{ssh}'")]);
    assert!(legacy_askpass_in(Some(&repo), &[("GIT_SSH_COMMAND", "")]));
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
