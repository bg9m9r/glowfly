use super::*;
use std::io::Cursor;

const SHORT: Duration = Duration::from_secs(5);

#[test]
fn the_go_signal_is_whole() {
    assert!(go_received(b"go\n"));
    assert!(go_received(b"go\nmore"));
    assert!(!go_received(b"go"), "partial");
    assert!(!go_received(b""));
    assert!(!go_received(b"no\n"));
}

#[test]
fn waiting_reads_go_then_the_end() {
    assert_eq!(wait_for_exit(Cursor::new(b"go\n".to_vec()), SHORT, SHORT), Wait::Exited);
    assert_eq!(wait_for_exit(Cursor::new(Vec::new()), SHORT, SHORT), Wait::NoGo);
    assert_eq!(wait_for_exit(Cursor::new(b"x".to_vec()), SHORT, SHORT), Wait::NoGo);
    assert_eq!(wait_for_exit(Cursor::new(b"go".to_vec()), SHORT, SHORT), Wait::NoGo);
}

/// Input that sends "go" and then never ends.
struct GoThenHang(bool);

impl Read for GoThenHang {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if std::mem::replace(&mut self.0, true) {
            std::thread::sleep(Duration::from_secs(3600));
            return Ok(0);
        }
        buf[..GO.len()].copy_from_slice(GO);
        Ok(GO.len())
    }
}

#[test]
fn waiting_gives_up_when_glowfly_stays() {
    assert_eq!(wait_for_exit(GoThenHang(false), SHORT, Duration::from_millis(50)), Wait::TimedOut);
}

#[test]
fn the_msiexec_command() {
    let root = crate::paths::temp_dir().join("Windows");
    let step = MsiStep { file: root.join("glowfly.msi"), log: root.join("install.log") };
    let (program, args) = msiexec_command(&root, &step);
    assert_eq!(program, root.join("System32").join("msiexec.exe"));
    let want: Vec<OsString> =
        vec!["/i".into(), step.file.clone().into(), "/passive".into(), "/norestart".into(), "/l*v".into(), step.log.clone().into()];
    assert_eq!(args, want);
}

#[test]
fn msiexec_exit_codes() {
    let cases = [
        (Some(0), MsiOutcome::Installed),
        (Some(3010), MsiOutcome::Installed),
        (Some(1641), MsiOutcome::Installed),
        (Some(1602), MsiOutcome::Cancelled),
        (Some(1618), MsiOutcome::Busy),
        (Some(1603), MsiOutcome::Failed(Some(1603))),
        (None, MsiOutcome::Failed(None)),
    ];
    for (code, want) in cases {
        assert_eq!(msi_outcome(code), want, "{code:?}");
    }
}

#[test]
fn without_go_the_helper_does_nothing() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-finish-nogo-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let backup = dir.join("glowfly.exe.bak");
    std::fs::write(&backup, "old").unwrap();
    let plan = Plan {
        format: super::super::plan::FORMAT,
        version: "0.3.0".into(),
        msi: None,
        remove: vec![backup.clone()],
        relaunch: dir.join("missing").join("glowfly.exe"),
        staging: dir.clone(),
    };
    let file = dir.join("plan.json");
    plan.write(&file).unwrap();
    let mut lines = Vec::new();
    let code = run_with(&file, Cursor::new(Vec::new()), (SHORT, SHORT), &mut |l| lines.push(l.to_owned()));
    assert_eq!(code, 1);
    assert!(backup.is_file() && file.is_file(), "nothing touched");
    assert_eq!(lines, ["not updating: Glowfly didn't hand over the update"]);
    // A plan that can't be read.
    let mut lines = Vec::new();
    assert_eq!(run_with(&dir.join("none.json"), Cursor::new(b"go\n".to_vec()), (SHORT, SHORT), &mut |l| lines.push(l.to_owned())), 1);
    assert!(lines[0].starts_with("not updating: Couldn't read "), "{lines:?}");
    let _ = crate::persist::remove_tree(&dir);
}

#[test]
fn after_go_and_exit_the_helper_finishes() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-finish-go-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&dir);
    let staging = dir.join("glowfly-update-0123456789abcdef");
    std::fs::create_dir_all(&staging).unwrap();
    let backup = dir.join("glowfly.exe.bak");
    std::fs::write(&backup, "old").unwrap();
    let plan = Plan {
        format: super::super::plan::FORMAT,
        version: "0.3.0".into(),
        msi: None,
        remove: vec![backup.clone()],
        // Doesn't exist, so nothing starts.
        relaunch: dir.join("missing").join("glowfly.exe"),
        staging: staging.clone(),
    };
    let file = staging.join("plan.json");
    plan.write(&file).unwrap();
    let mut lines = Vec::new();
    let code = run_with(&file, Cursor::new(b"go\n".to_vec()), (SHORT, SHORT), &mut |l| lines.push(l.to_owned()));
    assert_eq!(code, 0);
    assert!(!backup.exists() && !staging.exists(), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("couldn't start ")), "{lines:?}");
    let _ = crate::persist::remove_tree(&dir);
}
