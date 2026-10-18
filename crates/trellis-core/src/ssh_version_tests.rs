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
fn picks_the_ssh_git_runs() {
    fn os(s: &str) -> Option<&OsStr> {
        Some(OsStr::new(s))
    }
    assert_eq!(ssh_program(os("ssh -i key -o IdentitiesOnly=yes"), os("plink")), "ssh");
    assert_eq!(ssh_program(os("/opt/ssh/bin/ssh -v"), None), "/opt/ssh/bin/ssh");
    assert_eq!(ssh_program(os("  "), os("/usr/local/bin/ssh")), "/usr/local/bin/ssh", "an empty command is skipped");
    assert_eq!(ssh_program(os("'/my ssh/ssh' -v"), os("/usr/bin/ssh")), "/usr/bin/ssh", "a quoted program is skipped");
    assert_eq!(ssh_program(None, os("")), "ssh");
    assert_eq!(ssh_program(None, None), "ssh");
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
