//! The SSH agent: reading `ssh-add -l` and `sc.exe`, Pageant's pipes, where ssh-add is looked
//! for, and what ssh-add is given.

use super::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn key(bits: u32, fp: &str, comment: &str, kind: &str) -> AgentKey {
    AgentKey { bits: Some(bits), fingerprint: fp.into(), comment: comment.into(), kind: kind.into() }
}

#[test]
fn parses_agent_lines() {
    assert_eq!(
        parse_agent_line("256 SHA256:abc ada@x work laptop (ED25519)"),
        Some(key(256, "SHA256:abc", "ada@x work laptop", "ED25519"))
    );
    assert_eq!(parse_agent_line("3072 SHA256:def (RSA)"), Some(key(3072, "SHA256:def", "", "RSA")));
    assert_eq!(parse_agent_line("The agent has no identities."), None);
}

#[test]
fn reads_ssh_add_exit_codes() {
    let two = "256 SHA256:abc ada@x (ED25519)\n3072 SHA256:def work (RSA)\n";
    let AgentReach::Keys(keys) = parse_agent_list(Some(0), two, "") else { panic!("keys") };
    assert_eq!(keys, [key(256, "SHA256:abc", "ada@x", "ED25519"), key(3072, "SHA256:def", "work", "RSA")]);
    assert_eq!(parse_agent_list(Some(1), "The agent has no identities.\n", ""), AgentReach::Keys(vec![]));
    assert_eq!(parse_agent_list(Some(2), "", "Could not open a connection to your authentication agent.\n"), AgentReach::NoAgent);
    assert_eq!(parse_agent_list(Some(127), "", "\nboom\nmore\n"), AgentReach::Failed("boom".into()));
    assert_eq!(parse_agent_list(None, "", ""), AgentReach::Failed("ssh-add failed".into()));
}

#[test]
fn reads_the_windows_service() {
    let running = "SERVICE_NAME: ssh-agent\n        TYPE               : 10  WIN32_OWN_PROCESS\n        STATE              : 4  RUNNING\n";
    assert_eq!(parse_sc_query(Some(0), running), WinService::Running);
    let stopped = "SERVICE_NAME: ssh-agent\n        STATE              : 1  STOPPED\n";
    assert_eq!(parse_sc_query(Some(0), stopped), WinService::Stopped);
    let missing =
        "[SC] EnumQueryServicesStatus:OpenService FAILED 1060:\n\nThe specified service does not exist as an installed service.\n";
    assert_eq!(parse_sc_query(Some(1060), missing), WinService::Missing);
    assert_eq!(parse_sc_query(Some(1), missing), WinService::Missing);
    assert!(sc_start_disabled("        START_TYPE         : 4   DISABLED\n"));
    assert!(!sc_start_disabled("        START_TYPE         : 3   DEMAND_START\n"));
}

#[test]
fn recognises_pageant_pipes() {
    assert!(is_pageant_pipe("pageant.ada.abc123"));
    assert!(is_pageant_pipe("Pageant.Ada.x"));
    assert!(!is_pageant_pipe("openssh-ssh-agent"));
    assert!(!is_pageant_pipe("my.pageant.x"));
}

#[test]
fn ssh_add_candidate_order() {
    let path = std::env::join_paths([PathBuf::from("/a/bin")]).unwrap();
    let exec = Path::new("/Git/mingw64/libexec/git-core");
    let windows_own = PathBuf::from("/Windows").join("System32").join("OpenSSH").join("ssh-add.exe");
    let git_own = PathBuf::from("/Git").join("usr").join("bin").join("ssh-add.exe");
    let on_path = PathBuf::from("/a/bin").join("ssh-add.exe");
    let got = ssh_add_candidates_for(true, Some(&path), ".exe", Some(exec), Some(Path::new("/Windows")));
    assert_eq!(got, [windows_own.clone(), on_path.clone(), git_own.clone()], "Windows' own first: it reaches the service");
    let got = ssh_add_candidates_for(false, Some(&path), ".exe", Some(exec), Some(Path::new("/Windows")));
    assert_eq!(got, [on_path, git_own, windows_own], "PATH first elsewhere");
    assert_eq!(ssh_add_candidates_for(false, Some(&path), "", None, None), [PathBuf::from("/a/bin").join("ssh-add")]);
}

#[test]
fn ssh_add_gets_only_the_key_path() {
    let key = if cfg!(windows) { r"C:\Users\ada\.ssh\id_ed25519" } else { "/home/ada/.ssh/id_ed25519" };
    assert_eq!(add_args(Path::new(key)), [OsString::from(key)]);
}

#[test]
fn status_holds_keys_by_fingerprint() {
    let mut s = AgentStatus::default();
    assert!(!s.running() && !s.holds("SHA256:abc"));
    s.reach = Some(AgentReach::Keys(vec![key(256, "SHA256:abc", "", "ED25519")]));
    assert!(s.running() && s.holds("SHA256:abc") && !s.holds("SHA256:def"));
    s.reach = Some(AgentReach::NoAgent);
    assert!(!s.running());
}

#[test]
fn key_bodies_ignore_the_comment() {
    assert_eq!(key_body("ssh-ed25519 AAAAx ada@laptop\n"), Some(("ssh-ed25519", "AAAAx")));
    assert_eq!(key_body("ssh-ed25519 AAAAx"), key_body("ssh-ed25519   AAAAx other comment"));
    assert_ne!(key_body("ssh-ed25519 AAAAx"), key_body("ssh-rsa AAAAx"));
    assert_eq!(key_body("garbage AAAA"), None);
    assert_eq!(key_body("ssh-ed25519"), None);
}
