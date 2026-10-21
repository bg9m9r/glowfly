use super::*;

fn proc(pid: u32, parent: u32, exe: &str) -> ProcEntry {
    ProcEntry { pid, parent, exe: exe.into() }
}

fn running(name: &str) -> ShellActivity {
    ShellActivity::Running { program: Some(name.into()) }
}

#[test]
fn a_shell_without_children_is_idle() {
    let t = [proc(1, 0, "wininit.exe"), proc(10, 1, "pwsh.exe"), proc(11, 1, "git.exe")];
    assert_eq!(tree_activity(&t, 10), ShellActivity::Idle);
    assert_eq!(tree_activity(&t, 99), ShellActivity::Idle);
}

#[test]
fn a_child_is_the_running_program() {
    let t = [proc(10, 1, "pwsh.exe"), proc(20, 10, "git.exe"), proc(21, 20, "ssh.exe")];
    assert_eq!(tree_activity(&t, 10), running("git"));
}

#[test]
fn a_same_named_launcher_is_followed() {
    let chain = vec![proc(10, 1, "bash.exe"), proc(11, 10, "bash.exe")];
    assert_eq!(tree_activity(&chain, 10), ShellActivity::Idle);
    let mut busy = chain;
    busy.push(proc(12, 11, "vim.exe"));
    assert_eq!(tree_activity(&busy, 10), running("vim"));
}

#[test]
fn console_hosts_are_ignored() {
    let t = [proc(10, 1, "cmd.exe"), proc(11, 10, "conhost.exe"), proc(12, 10, "OpenConsole.exe")];
    assert_eq!(tree_activity(&t, 10), ShellActivity::Idle);
    let t = [proc(10, 1, "cmd.exe"), proc(11, 10, "CONHOST.EXE"), proc(12, 10, "ping.exe")];
    assert_eq!(tree_activity(&t, 10), running("ping"));
}

#[test]
fn exe_names_compare_ignoring_case() {
    let t = [proc(10, 1, "BASH.EXE"), proc(11, 10, "bash.exe")];
    assert_eq!(tree_activity(&t, 10), ShellActivity::Idle);
}

#[test]
fn self_parents_and_deep_chains_end() {
    // The idle process is its own parent.
    let t = [proc(0, 0, "[System Process]"), proc(4, 0, "System")];
    assert_eq!(tree_activity(&t, 0), running("System"));
    let t = [proc(0, 0, "[System Process]")];
    assert_eq!(tree_activity(&t, 0), ShellActivity::Idle);
    // Five same-named levels below the shell: the depth limit stops before the program at the
    // bottom is reached.
    let mut deep: Vec<ProcEntry> = (0..6).map(|i| proc(10 + i, 9 + i, "bash.exe")).collect();
    deep.push(proc(16, 15, "vim.exe"));
    assert_eq!(tree_activity(&deep, 10), ShellActivity::Idle);
    assert_eq!(tree_activity(&deep, 12), running("vim"));
}

#[test]
fn foreground_group_against_the_shell() {
    let name = |pid: i32| Some(format!("p{pid}"));
    assert_eq!(foreground_activity(Some(42), Some(42), name), ShellActivity::Idle);
    assert_eq!(foreground_activity(Some(43), Some(42), name), running("p43"));
    assert_eq!(foreground_activity(Some(43), Some(42), |_| None), ShellActivity::Running { program: None });
    assert_eq!(foreground_activity(None, Some(42), name), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(0), Some(42), name), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(-1), Some(42), name), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(42), None, name), ShellActivity::Unknown);
}

/// A real shell goes from Idle to Running while it runs a program.
#[test]
fn a_live_shell_reports_its_program() {
    use crate::terminal::pty::{ShellCommand, spawn};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (shell, command, program) = if cfg!(windows) {
        (ShellCommand { program: "cmd.exe".into(), args: vec![] }, "ping -n 30 127.0.0.1\r", Some("ping"))
    } else {
        if !std::path::Path::new("/bin/bash").exists() {
            eprintln!("skipped: no /bin/bash");
            return;
        }
        let args = ["--noprofile", "--norc", "-i"].map(String::from).to_vec();
        let name = cfg!(target_os = "linux").then_some("sleep");
        (ShellCommand { program: "/bin/bash".into(), args }, "sleep 30\n", name)
    };
    let dir = crate::paths::temp_dir().join(format!("trellis-activity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (out_tx, out_rx) = mpsc::channel();
    let mut session = spawn(
        &dir,
        24,
        80,
        &shell,
        move |b| {
            let _ = out_tx.send(b);
        },
        |_| {},
    )
    .expect("spawn");
    let probe = session.probe();
    let mut screen = crate::terminal::TermScreen::new(24, 80);
    // Answers the pseudoconsole's queries until `want` holds or the deadline passes.
    let mut wait_for = |session: &mut crate::terminal::pty::PtySession, want: &dyn Fn(&ShellActivity) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            while let Ok(b) = out_rx.try_recv() {
                let reply = screen.feed(&b);
                if !reply.is_empty() {
                    session.write(&reply).expect("reply");
                }
            }
            let now = probe.activity();
            if want(&now) {
                return now;
            }
            assert!(Instant::now() < deadline, "still {now:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    wait_for(&mut session, &|a| *a == ShellActivity::Idle);
    session.write(command.as_bytes()).expect("write");
    let busy = wait_for(&mut session, &|a| matches!(a, ShellActivity::Running { .. }));
    let ShellActivity::Running { program: got } = busy else { unreachable!() };
    match program {
        Some(p) => assert!(got.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(p)), "{got:?}"),
        None => assert_eq!(got, None),
    }
    let _ = session.killer().kill();
    drop(session);
    let _ = std::fs::remove_dir_all(&dir);
}
