//! `git_procs`: which names are git, which daemons are left out and only when they say so,
//! that every unreadable thing refuses, and a real git process being found.

use super::*;

fn p(pid: u32, name: &str) -> Proc {
    (pid, Some(name.to_owned()))
}

fn no_args(_: u32) -> Option<Vec<String>> {
    None
}

fn words(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

#[test]
fn git_and_its_helpers_count() {
    for name in ["git", "git.exe", "GIT.EXE", "git-remote-https", "git-lfs.exe", "git-credential-manager.exe"] {
        assert!(is_git(name), "{name}");
    }
    for name in ["gitk", "github", "GitHubDesktop.exe", "git-", "glowfly.exe", "legit", "bash.exe", ""] {
        assert!(!is_git(name), "{name}");
    }
}

#[test]
fn the_verdict_errs_on_refusing() {
    assert_eq!(judge(vec![p(1, "init"), p(7, "bash")], 99, no_args), GitProcesses::None);
    let found = judge(vec![p(1, "init"), p(30, "git"), p(20, "git-remote-https"), (5, None)], 99, no_args);
    let both = GitProcesses::Running(vec![(20, "git-remote-https".into()), (30, "git".into())]);
    assert_eq!(found, both, "git is told even beside an unreadable name");
    assert_eq!(judge(vec![p(1, "init"), (5, None)], 99, no_args), GitProcesses::Unknown(Unknown::Unreadable));
    assert_eq!(judge(vec![p(99, "git")], 99, no_args), GitProcesses::None, "Glowfly itself is left out");
    let many: Vec<Proc> = (10..20).map(|i| p(i, "git")).collect();
    let GitProcesses::Running(named) = judge(many, 1, no_args) else { panic!("running") };
    assert_eq!(named.len(), SHOWN);
}

#[test]
fn only_daemons_that_say_so_are_left_out() {
    let lines = |pid: u32| match pid {
        1 => Some(words("git credential-cache--daemon /run/user/1000/glowfly/cache")),
        2 => Some(split_command_line(r#""C:\Program Files\Git\mingw64\bin\git.exe" -c core.x=1 fsmonitor--daemon run --detach"#)),
        3 => Some(words("git commit -m fsmonitor--daemon")),
        _ => None,
    };
    let daemons = vec![p(1, "git"), p(2, "git.exe"), p(4, "git-credential-cache--daemon")];
    assert_eq!(judge(daemons, 99, lines), GitProcesses::None);
    assert_eq!(judge(vec![p(3, "git")], 99, lines), GitProcesses::Running(vec![(3, "git".into())]), "a commit is no daemon");
    assert_eq!(judge(vec![p(5, "git")], 99, lines), GitProcesses::Running(vec![(5, "git".into())]), "an unread command line counts");
    assert!(!is_daemon("git", Some(&words("git -C fsmonitor--daemon status"))), "-C takes a value");
    assert!(!is_daemon("git", Some(&[])));
    assert!(!is_daemon("git-remote-https", None));
}

#[test]
fn windows_command_lines_split_as_git_sees_them() {
    assert_eq!(split_command_line(r#""C:\a b\git.exe"  fetch "x y" "#), [r"C:\a b\git.exe", "fetch", "x y"]);
    assert_eq!(split_command_line(r#"git -m "say \"hi\"" """#), ["git", "-m", r#"say "hi""#, ""]);
}

#[test]
fn an_open_wsl_session_is_unknown_on_windows() {
    let v = judge(vec![p(1, "explorer.exe"), p(2, "wslhost.exe")], 99, no_args);
    assert_eq!(v, if cfg!(windows) { GitProcesses::Unknown(Unknown::Wsl) } else { GitProcesses::None });
}

#[test]
fn hidepid_hides_other_users_processes() {
    assert!(proc_hidden("proc /proc proc rw,nosuid,nodev,noexec,relatime,hidepid=2 0 0\n"));
    assert!(proc_hidden("proc /proc proc rw,hidepid=invisible 0 0"));
    assert!(!proc_hidden("proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\nsysfs /sys sysfs rw 0 0"));
    assert!(!proc_hidden("proc /proc proc rw,hidepid=0 0 0"));
    assert!(!proc_hidden("proc /mnt/x proc rw,hidepid=2 0 0"), "another mount of proc");
}

#[test]
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn a_running_git_and_its_command_line_are_found() {
    use std::process::{Command, Stdio};
    // Waits on its input until it is closed.
    let mut child = Command::new("git").args(["hash-object", "--stdin"]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
    let v = scan();
    let line = os::args(child.id());
    drop(child.stdin.take());
    let _ = child.wait();
    match v {
        GitProcesses::Running(_) => {}
        GitProcesses::Unknown(why) => assert_ne!(why, Unknown::NoList, "the list is read here"),
        GitProcesses::None => panic!("git {} wasn't found", child.id()),
    }
    let line = line.expect("its own child's command line is readable");
    assert!(line.iter().any(|a| a == "hash-object"), "{line:?}");
}
