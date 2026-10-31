//! Glowfly as git's askpass program: the built executable, run with the listener's address and
//! token in its environment, asks a listener started here and prints the answer, both on its
//! own and when git runs it during `git credential fill`.

use glowfly_core::askpass::server::{Event, Server, start};
use glowfly_core::askpass::{ENV_ADDR, ENV_TOKEN, PromptKind, Reply, SSH_HINT_ENV, Secret};
use glowfly_core::gitcli::{self, AskpassEnv, args};
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EXE: &str = env!("CARGO_BIN_EXE_glowfly");

/// A listener that answers every prompt with `answer(kind)` (None cancels), recording the
/// prompts it saw.
fn listener(answer: fn(&PromptKind) -> Option<&'static str>) -> (Arc<Server>, Receiver<PromptKind>) {
    let (tx, rx) = mpsc::channel::<Event>();
    let tx = Mutex::new(tx);
    let server = start(move |e| {
        let _ = tx.lock().unwrap().send(e);
    })
    .unwrap();
    let (seen_tx, seen) = mpsc::channel();
    let s = server.clone();
    std::thread::spawn(move || {
        for e in rx {
            if let Event::Asked { id, prompt } = e {
                let reply = answer(&prompt.kind).map_or(Reply::Cancel, |a| Reply::Answer(Secret::new(a)));
                let _ = seen_tx.send(prompt.kind);
                s.answer(id, reply);
            }
        }
    });
    (server, seen)
}

fn by_kind(kind: &PromptKind) -> Option<&'static str> {
    match kind {
        PromptKind::Username { .. } => Some("ada"),
        PromptKind::Password { .. } => Some("hunter2"),
        _ => None,
    }
}

/// Runs the executable with `argv` and only the given askpass variables.
fn helper(argv: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(EXE);
    cmd.args(argv);
    for var in [ENV_ADDR, ENV_TOKEN, SSH_HINT_ENV] {
        cmd.env_remove(var);
    }
    cmd.envs(env.iter().copied());
    cmd.output().unwrap()
}

#[test]
fn the_helper_prints_the_answer() {
    let (s, seen) = listener(by_kind);
    let (addr, token) = (s.addr(), s.token().to_owned());
    let env = [(ENV_ADDR, addr.as_str()), (ENV_TOKEN, token.as_str())];
    let prompt = "Password for 'https://ada@example.com': ";
    for argv in [vec![prompt], vec!["--askpass", prompt]] {
        let out = helper(&argv, &env);
        assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"), "hunter2\n");
        assert!(matches!(seen.recv_timeout(Duration::from_secs(10)).unwrap(), PromptKind::Password { .. }));
    }
}

#[test]
fn the_helper_fails_without_an_answer() {
    let (s, seen) = listener(|_| None);
    let (addr, token) = (s.addr(), s.token().to_owned());
    let out = helper(&["Username for 'https://example.com': "], &[(ENV_ADDR, &addr), (ENV_TOKEN, &token)]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cancelled"));
    seen.recv_timeout(Duration::from_secs(10)).unwrap();

    let wrong = "0".repeat(32);
    let out = helper(&["Username for 'https://example.com': "], &[(ENV_ADDR, &addr), (ENV_TOKEN, &wrong)]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty());

    let out = helper(&["--askpass", "Username for 'https://example.com': "], &[]);
    assert_eq!(out.status.code(), Some(2), "no listener configured");

    // A notice (a security-key touch) needs no answer.
    let out = helper(&["Confirm user presence for key"], &[(ENV_ADDR, &addr), (ENV_TOKEN, &token), (SSH_HINT_ENV, "none")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    assert!(seen.recv_timeout(Duration::from_millis(500)).is_err(), "the listener was never asked");
}

#[test]
fn git_asks_through_the_helper() {
    let (s, seen) = listener(by_kind);
    let env = AskpassEnv { exe: EXE.into(), addr: s.addr(), token: s.token().into() };
    let vars: Vec<(&str, String)> = gitcli::askpass_vars(&env).into_iter().map(|(k, v)| (k, os(v))).collect();
    let mut env: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
    env.push(("GCM_INTERACTIVE", "never"));
    let dir = glowfly_core::paths::temp_dir();
    let fill = args(&["-c", "credential.helper=", "credential", "fill"]);
    let input = "protocol=https\nhost=example.invalid\n\n";

    let (ok, out) = gitcli::run_with_input(Path::new(&dir), &fill, input, &env);
    assert!(ok, "{out}");
    assert!(out.lines().any(|l| l == "username=ada"), "{out}");
    assert!(out.lines().any(|l| l == "password=hunter2"), "{out}");
    let wait = Duration::from_secs(10);
    assert_eq!(seen.recv_timeout(wait).unwrap(), PromptKind::Username { url: "https://example.invalid".into() });
    assert!(matches!(seen.recv_timeout(wait).unwrap(), PromptKind::Password { user: Some(u), .. } if u == "ada"));

    env.extend_from_slice(gitcli::NO_PROMPTS);
    let (ok, out) = gitcli::run_with_input(Path::new(&dir), &fill, input, &env);
    assert!(!ok, "{out}");
    assert!(seen.recv_timeout(Duration::from_millis(500)).is_err(), "no prompt with NO_PROMPTS");
}

fn os(v: OsString) -> String {
    v.into_string().expect("UTF-8 test paths")
}
