use super::*;

fn proc(pid: u32, parent: u32, exe: &str) -> ProcEntry {
    ProcEntry { pid, parent, exe: exe.into() }
}

fn running(name: &str) -> ShellActivity {
    ShellActivity::Running { program: Some(name.into()) }
}

/// No process's image path is known.
fn no_image(_: u32) -> Option<String> {
    None
}

fn tree(procs: &[ProcEntry], shell: u32) -> ShellActivity {
    tree_activity(procs, shell, no_image)
}

/// No shell has children.
fn no_kids(_: u32) -> Vec<i32> {
    Vec::new()
}

#[test]
fn a_shell_without_children_is_idle() {
    let t = [proc(1, 0, "wininit.exe"), proc(10, 1, "pwsh.exe"), proc(11, 1, "git.exe")];
    assert_eq!(tree(&t, 10), ShellActivity::Idle);
    assert_eq!(tree(&t, 99), ShellActivity::Idle);
}

#[test]
fn a_child_is_the_running_program() {
    let t = [proc(10, 1, "pwsh.exe"), proc(20, 10, "git.exe"), proc(21, 20, "ssh.exe")];
    assert_eq!(tree(&t, 10), running("git"));
}

#[test]
fn a_same_named_launcher_is_followed() {
    let chain = vec![proc(10, 1, "bash.exe"), proc(11, 10, "bash.exe")];
    assert_eq!(tree(&chain, 10), ShellActivity::Idle);
    let mut busy = chain;
    busy.push(proc(12, 11, "vim.exe"));
    assert_eq!(tree(&busy, 10), running("vim"));
}

#[test]
fn console_hosts_are_ignored() {
    let t = [proc(10, 1, "cmd.exe"), proc(11, 10, "conhost.exe"), proc(12, 10, "OpenConsole.exe")];
    assert_eq!(tree(&t, 10), ShellActivity::Idle);
    let t = [proc(10, 1, "cmd.exe"), proc(11, 10, "CONHOST.EXE"), proc(12, 10, "ping.exe")];
    assert_eq!(tree(&t, 10), running("ping"));
}

#[test]
fn exe_names_compare_ignoring_case() {
    let t = [proc(10, 1, "BASH.EXE"), proc(11, 10, "bash.exe")];
    assert_eq!(tree(&t, 10), ShellActivity::Idle);
}

#[test]
fn self_parents_and_deep_chains_end() {
    // The idle process is its own parent.
    let t = [proc(0, 0, "[System Process]"), proc(4, 0, "System")];
    assert_eq!(tree(&t, 0), running("System"));
    let t = [proc(0, 0, "[System Process]")];
    assert_eq!(tree(&t, 0), ShellActivity::Idle);
    // Five same-named levels below the shell: the depth limit stops before the program at the
    // bottom is reached.
    let mut deep: Vec<ProcEntry> = (0..6).map(|i| proc(10 + i, 9 + i, "bash.exe")).collect();
    deep.push(proc(16, 15, "vim.exe"));
    assert_eq!(tree(&deep, 10), ShellActivity::Idle);
    assert_eq!(tree(&deep, 12), running("vim"));
}

#[test]
fn foreground_group_against_the_shell() {
    let name = |pid: i32| Some(format!("p{pid}"));
    assert_eq!(foreground_activity(Some(42), Some(42), name, no_kids), ShellActivity::Idle);
    assert_eq!(foreground_activity(Some(43), Some(42), name, no_kids), running("p43"));
    assert_eq!(foreground_activity(Some(43), Some(42), |_| None, no_kids), ShellActivity::Running { program: None });
    assert_eq!(foreground_activity(None, Some(42), name, no_kids), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(0), Some(42), name, no_kids), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(-1), Some(42), name, no_kids), ShellActivity::Unknown);
    assert_eq!(foreground_activity(Some(42), None, name, no_kids), ShellActivity::Unknown);
}

#[test]
fn a_same_named_child_from_the_same_program_is_a_job_not_a_launcher() {
    // PowerShell's Start-Job runs another pwsh.exe from the same file.
    let t = [proc(10, 1, "pwsh.exe"), proc(11, 10, "pwsh.exe")];
    let image = |_: u32| Some(r"C:\Program Files\PowerShell\7\pwsh.exe".to_owned());
    assert_eq!(tree_activity(&t, 10, image), running("pwsh"));
    // Git Bash's launcher (bin) starts the real bash (usr\bin): followed.
    let image = |pid: u32| Some(if pid == 10 { r"C:\Git\bin\bash.exe" } else { r"C:\Git\usr\bin\bash.exe" }.to_owned());
    let t = [proc(10, 1, "bash.exe"), proc(11, 10, "bash.exe")];
    assert_eq!(tree_activity(&t, 10, image), ShellActivity::Idle);
    // Paths compare ignoring case; an unknown path keeps following.
    let t = [proc(10, 1, "pwsh.exe"), proc(11, 10, "pwsh.exe")];
    let image = |pid: u32| Some(if pid == 10 { r"C:\PS\pwsh.exe" } else { r"c:\ps\PWSH.EXE" }.to_owned());
    assert_eq!(tree_activity(&t, 10, image), running("pwsh"));
    let image = |pid: u32| (pid == 10).then(|| r"C:\PS\pwsh.exe".to_owned());
    assert_eq!(tree_activity(&t, 10, image), ShellActivity::Idle);
}

#[test]
fn a_background_job_of_an_idle_bash_is_running() {
    // Git Bash at its prompt with `sleep 30 &`.
    let t = [proc(10, 1, "bash.exe"), proc(11, 10, "bash.exe"), proc(12, 11, "sleep.exe")];
    assert_eq!(tree(&t, 10), running("sleep"));
}

/// wsl.exe (System32) starts wsl.exe (Program Files\WSL), which keeps wslhost.exe and a console
/// host; programs inside the distribution don't show, Windows programs started there do.
#[test]
fn wsl_helpers_are_ignored_and_its_shell_is_unknown() {
    let image = |pid: u32| Some(if pid == 10 { r"C:\Windows\System32\wsl.exe" } else { r"C:\Program Files\WSL\wsl.exe" }.to_owned());
    let mut t = vec![
        proc(10, 1, "wsl.exe"),
        proc(11, 10, "conhost.exe"),
        proc(12, 10, "wsl.exe"),
        proc(13, 12, "wslhost.exe"),
        proc(14, 13, "conhost.exe"),
    ];
    assert_eq!(tree_activity(&t, 10, image), ShellActivity::Unknown);
    t.push(proc(15, 12, "PING.EXE"));
    assert_eq!(tree_activity(&t, 10, image), running("PING"));
    // A plain shell with no children is still Idle.
    assert_eq!(tree(&[proc(10, 1, "cmd.exe")], 10), ShellActivity::Idle);
}

#[test]
fn background_jobs_of_a_shell_in_the_foreground_are_running() {
    let name = |pid: i32| Some(format!("p{pid}"));
    assert_eq!(foreground_activity(Some(42), Some(42), name, |_| vec![50, 51]), running("p50"));
    assert_eq!(foreground_activity(Some(42), Some(42), |_| None, |_| vec![50]), ShellActivity::Running { program: None });
    // A foreground program is named first.
    assert_eq!(foreground_activity(Some(43), Some(42), name, |_| vec![50]), running("p43"));
}

#[test]
fn proc_stat_lines_parse() {
    let s = parse_stat("1234 (bash) S 1000 1234 1234 34816 1300 4194560 ...").unwrap();
    assert_eq!((s.state, s.ppid, s.session), ('S', 1000, 1234));
    // A command name with spaces and parentheses ends at the last ')'.
    let s = parse_stat("77 (my (odd) prog) Z 42 77 9 0").unwrap();
    assert_eq!((s.state, s.ppid, s.session), ('Z', 42, 9));
    assert!(parse_stat("").is_none());
    assert!(parse_stat("12 (x) S").is_none());
    assert!(parse_stat("12 x S 1 2 3").is_none());
}

#[test]
fn children_are_those_of_the_shell_in_its_session_and_not_ended() {
    let stat = |ppid, session, state| Stat { state, ppid, session };
    let all = [(50, stat(42, 7, 'S')), (51, stat(42, 7, 'Z')), (52, stat(42, 99, 'S')), (53, stat(1, 7, 'R')), (54, stat(42, 7, 'R'))];
    assert_eq!(children_in_session(&all, 42, 7), vec![50, 54]);
}

/// A real shell goes from Idle to Running while it runs a program.
#[test]
fn a_live_shell_reports_its_program() {
    if cfg!(windows) {
        live_shell("fg", "ping -n 30 127.0.0.1\r", "ping");
    } else {
        live_shell("fg", "sleep 30\n", "sleep");
    }
}

/// A background job counts as running while the shell waits at its prompt.
#[test]
fn a_live_shell_reports_its_background_job() {
    if cfg!(windows) {
        live_shell("bg", "start /b ping -n 30 127.0.0.1 >nul\r", "ping");
    } else {
        live_shell("bg", "sleep 30 &\n", "sleep");
    }
}

/// Starts cmd.exe (Windows) or bash, waits for Idle, types `command`, then waits for Running
/// and checks the program's name where the system tells it.
fn live_shell(tag: &str, command: &str, program: &str) {
    use crate::terminal::pty::{ShellCommand, spawn};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let shell = if cfg!(windows) {
        ShellCommand { program: "cmd.exe".into(), args: vec![] }
    } else {
        if !std::path::Path::new("/bin/bash").exists() {
            eprintln!("skipped: no /bin/bash");
            return;
        }
        let args = ["--noprofile", "--norc", "-i"].map(String::from).to_vec();
        ShellCommand { program: "/bin/bash".into(), args }
    };
    let program = cfg!(any(windows, target_os = "linux", target_os = "macos")).then_some(program);
    let dir = crate::paths::temp_dir().join(format!("glowfly-activity-{tag}-{}", std::process::id()));
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
