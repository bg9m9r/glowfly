//! Detached starts' Windows flags, and trying a copied helper: it must exit successfully, in
//! time, printing what was expected.

use super::*;

#[test]
fn the_windows_flags() {
    assert_eq!(creation_flags(true), 0x0100_0208);
    assert_eq!(creation_flags(false), 0x0000_0208);
}

/// `script` run by the system's shell (cmd on Windows, sh elsewhere).
fn shell(script: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new(std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into()));
        c.arg("/C").arg(script);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(script);
        c
    }
}

const SHORT: Duration = Duration::from_secs(10);

#[test]
fn a_probe_needs_the_expected_output_and_success() {
    probe(shell("echo glowfly 1.2.3"), "glowfly 1.2.3", SHORT).unwrap();
    let wrong = probe(shell("echo something else"), "glowfly 1.2.3", SHORT).unwrap_err();
    assert!(wrong.to_string().contains("something else"), "{wrong}");
    let failed = probe(shell("echo glowfly 1.2.3 && exit 3"), "glowfly 1.2.3", SHORT).unwrap_err();
    assert!(failed.to_string().starts_with("it exited with"), "{failed}");
    let missing = probe(Command::new(crate::paths::temp_dir().join("glowfly-no-such-program")), "x", SHORT).unwrap_err();
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn a_probe_that_hangs_is_killed() {
    #[cfg(windows)]
    let script = "ping -n 30 127.0.0.1 >NUL";
    #[cfg(not(windows))]
    let script = "sleep 30";
    let started = Instant::now();
    let err = probe(shell(script), "glowfly 1.2.3", Duration::from_millis(300)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

/// The real try on Unix: a script that answers like this build runs, and one that doesn't, or
/// a file without the executable bit (as on a noexec mount), doesn't.
#[cfg(unix)]
#[test]
fn trying_a_copy_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::paths::temp_dir().join(format!("glowfly-try-run-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = |name: &str, body: &str, mode: u32| {
        let p = dir.join(name);
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh
{body}
"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        p
    };
    let own = format!("echo glowfly {}", super::super::CURRENT_VERSION);
    try_run(&script("runs", &own, 0o755)).unwrap();
    assert!(try_run(&script("old", "echo glowfly 0.0.1", 0o755)).is_err());
    assert!(try_run(&script("fails", &format!("{own}; exit 127"), 0o755)).is_err());
    assert_eq!(try_run(&script("noexec", &own, 0o644)).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    let _ = crate::persist::remove_tree(&dir);
}
