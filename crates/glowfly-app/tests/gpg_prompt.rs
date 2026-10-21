//! Trellis as git's gpg.program: the built executable stops a loop back into itself, still
//! answers askpass prompts with the wrapper marker set, and (on Unix, with a fake gpg script)
//! asks a listener started here for the signing passphrase while git commits; with a protected
//! SSH key, ssh-keygen's prompt reaches the listener as an SSH signing prompt.

use glowfly_core::askpass::server::{Event, Server, start};
use glowfly_core::askpass::{ENV_ADDR, ENV_TOKEN, PromptKind, Reply, SSH_HINT_ENV, Secret};
use glowfly_core::gpg_prompt::{ENV_WRAPPER, LOOP_MESSAGE};
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EXE: &str = env!("CARGO_BIN_EXE_glowfly");

/// A listener answering the prompts it sees with `answers` in turn (None cancels), recording
/// each prompt's kind.
fn listener(answers: Vec<Option<&'static str>>) -> (Arc<Server>, Receiver<PromptKind>) {
    let (tx, rx) = mpsc::channel::<Event>();
    let tx = Mutex::new(tx);
    let server = start(move |e| {
        let _ = tx.lock().unwrap().send(e);
    })
    .unwrap();
    let (seen_tx, seen) = mpsc::channel();
    let s = server.clone();
    std::thread::spawn(move || {
        let mut answers = answers.into_iter();
        for e in rx {
            if let Event::Asked { id, prompt } = e {
                let reply = answers.next().flatten().map_or(Reply::Cancel, |a| Reply::Answer(Secret::new(a)));
                let _ = seen_tx.send(prompt.kind);
                s.answer(id, reply);
            }
        }
    });
    (server, seen)
}

fn trellis(argv: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(EXE);
    cmd.args(argv);
    for var in [ENV_ADDR, ENV_TOKEN, SSH_HINT_ENV, ENV_WRAPPER] {
        cmd.env_remove(var);
    }
    cmd.envs(env.iter().copied());
    cmd.output().unwrap()
}

#[test]
fn a_gpg_program_that_leads_back_to_trellis_stops() {
    let out = trellis(&["--status-fd=2", "-bsau", "K"], &[(ENV_WRAPPER, "nested")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains(LOOP_MESSAGE), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty());
}

#[test]
fn askpass_prompts_still_work_with_the_marker_set() {
    let (s, seen) = listener(vec![Some("hunter2")]);
    let (addr, token) = (s.addr(), s.token().to_owned());
    let env = [(ENV_ADDR, addr.as_str()), (ENV_TOKEN, token.as_str()), (ENV_WRAPPER, "1")];
    let out = trellis(&["Password for 'https://ada@example.com': "], &env);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"), "hunter2\n");
    assert!(matches!(seen.recv_timeout(Duration::from_secs(10)).unwrap(), PromptKind::Password { .. }));
}

#[cfg(unix)]
mod end_to_end {
    use super::*;
    use glowfly_core::gitcli::{self, AskpassEnv, args};
    use glowfly_core::signing::SigningFormat;
    use std::path::{Path, PathBuf};

    /// A stand-in for gpg: lists one key, refuses `--pinentry-mode error` with "No pinentry",
    /// and in loopback mode signs only when the first stdin line is "right".
    const FAKE_GPG: &str = r#"#!/bin/sh
mode=plain
list=
prev=
for a in "$@"; do
  case "$a" in
    --list-secret-keys) list=1 ;;
    error|loopback) [ "$prev" = "--pinentry-mode" ] && mode=$a ;;
  esac
  prev=$a
done
if [ -n "$list" ]; then
  printf 'sec:u:4096:1:ABCDEF0123456789:1700000000:::u:::scESC:::+:::23::0:\n'
  printf 'uid:u::::1700000000::0000::Ada Lovelace <ada@example.com>::::::::::0:\n'
  exit 0
fi
case "$mode" in
  error)
    cat >/dev/null
    printf 'gpg: signing failed: No pinentry\n[GNUPG:] FAILURE sign 67108949\n' >&2
    exit 2 ;;
  loopback)
    IFS= read -r pass
    cat >/dev/null
    if [ "$pass" = right ]; then
      printf '[GNUPG:] KEY_CONSIDERED ABCDEF0123456789 2\n[GNUPG:] BEGIN_SIGNING H10\n[GNUPG:] SIG_CREATED D 1 10 00 1 ABCDEF0123456789\n' >&2
      printf -- '-----BEGIN PGP SIGNATURE-----\n\nZmFrZSBzaWduYXR1cmU=\n-----END PGP SIGNATURE-----\n'
      exit 0
    fi
    printf '[GNUPG:] BAD_PASSPHRASE ABCDEF0123456789\n[GNUPG:] FAILURE sign 67108875\ngpg: signing failed: Bad passphrase\n' >&2
    exit 2 ;;
  *)
    cat >/dev/null
    echo 'gpg: no pinentry in this test' >&2
    exit 2 ;;
esac
"#;

    fn scratch(name: &str) -> PathBuf {
        let dir = glowfly_core::paths::temp_dir().join(format!("trellis-gpg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("fake-gpg");
        std::fs::write(&fake, FAKE_GPG).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let fake = fake.to_str().unwrap();
        for a in [
            &["init", "-q"][..],
            &["config", "gpg.format", "openpgp"],
            &["config", "gpg.program", fake],
            &["config", "user.signingkey", "ABCDEF0123456789"],
        ] {
            let (ok, out) = gitcli::run(&repo, &args(a));
            assert!(ok, "{out}");
        }
        repo
    }

    /// `git commit -S` with the askpass and wrapper variables a foreground job gets.
    fn commit(repo: &Path, s: &Server) -> gitcli::GitOutput {
        let askpass = AskpassEnv { exe: EXE.into(), addr: s.addr(), token: s.token().into() };
        let mut vars: Vec<(&str, String)> =
            gitcli::askpass_vars(&askpass).into_iter().map(|(k, v)| (k, v.into_string().unwrap())).collect();
        vars.extend(
            glowfly_core::gpg_prompt::env::vars(Path::new(EXE), None, &glowfly_core::git_program::program())
                .into_iter()
                .map(|(k, v)| (k, v.into_string().unwrap())),
        );
        let mut env: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
        env.extend([
            ("GIT_AUTHOR_NAME", "Ada Lovelace"),
            ("GIT_AUTHOR_EMAIL", "ada@example.com"),
            ("GIT_COMMITTER_NAME", "Ada Lovelace"),
            ("GIT_COMMITTER_EMAIL", "ada@example.com"),
        ]);
        gitcli::output(repo, &args(&["commit", "-S", "--allow-empty", "-m", "x"]), "", &env, Some(Duration::from_secs(60)))
    }

    fn gpg_kind(retry: bool) -> PromptKind {
        PromptKind::GpgPassphrase { key: "ABCDEF0123456789".into(), user: Some("Ada Lovelace <ada@example.com>".into()), retry }
    }

    #[test]
    fn git_signs_after_a_wrong_then_right_passphrase() {
        let repo = scratch("sign");
        let (s, seen) = listener(vec![Some("wrong"), Some("right")]);
        let out = commit(&repo, &s);
        assert!(out.ok, "{}", out.combined());
        let wait = Duration::from_secs(10);
        assert_eq!(seen.recv_timeout(wait).unwrap(), gpg_kind(false));
        assert_eq!(seen.recv_timeout(wait).unwrap(), gpg_kind(true));
        assert!(seen.recv_timeout(Duration::from_millis(300)).is_err(), "asked twice");
        let (ok, body) = gitcli::run(&repo, &args(&["cat-file", "commit", "HEAD"]));
        assert!(ok && body.contains("ZmFrZSBzaWduYXR1cmU="), "{body}");
        let _ = std::fs::remove_dir_all(repo.parent().unwrap());
    }

    #[test]
    fn a_cancelled_prompt_fails_the_commit() {
        let repo = scratch("cancel");
        let (s, seen) = listener(vec![None]);
        let out = commit(&repo, &s);
        assert!(!out.ok, "{}", out.combined());
        assert!(out.stderr.contains("the passphrase prompt was cancelled"), "{}", out.stderr);
        assert_eq!(seen.recv_timeout(Duration::from_secs(10)).unwrap(), gpg_kind(false));
        let _ = std::fs::remove_dir_all(repo.parent().unwrap());
    }

    /// A repository signing with a new ed25519 key protected by the passphrase "right": the
    /// repository and the key's public file, or None (after saying so) without ssh-keygen.
    fn ssh_scratch(name: &str) -> Option<(PathBuf, PathBuf)> {
        let dir = glowfly_core::paths::temp_dir().join(format!("trellis-gpg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let made = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "right", "-C", "t", "-f"])
            .arg(dir.join("key"))
            .stdin(std::process::Stdio::null())
            .output();
        match made {
            Ok(o) if o.status.success() => {}
            other => {
                eprintln!("ssh-keygen unavailable, skipping: {other:?}");
                return None;
            }
        }
        let (repo, public) = (dir.join("repo"), dir.join("key.pub"));
        std::fs::create_dir_all(&repo).unwrap();
        let public_text = public.to_str().unwrap();
        for a in [&["init", "-q"][..], &["config", "gpg.format", "ssh"], &["config", "user.signingkey", public_text]] {
            let (ok, out) = gitcli::run(&repo, &args(a));
            assert!(ok, "{out}");
        }
        Some((repo, public))
    }

    /// `git commit -S` with the askpass variables and the SSH signing marker a foreground job
    /// gets, and no ssh-agent.
    fn ssh_commit(repo: &Path, s: &Server, public: &Path) -> gitcli::GitOutput {
        let askpass = AskpassEnv { exe: EXE.into(), addr: s.addr(), token: s.token().into() };
        let mut vars: Vec<(&str, String)> =
            gitcli::askpass_vars(&askpass).into_iter().map(|(k, v)| (k, v.into_string().unwrap())).collect();
        vars.extend(
            glowfly_core::gpg_prompt::ssh_env::vars(SigningFormat::Ssh, public.to_str())
                .into_iter()
                .map(|(k, v)| (k, v.into_string().unwrap())),
        );
        let mut env: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
        env.extend([
            ("SSH_AUTH_SOCK", ""),
            ("GIT_AUTHOR_NAME", "Ada Lovelace"),
            ("GIT_AUTHOR_EMAIL", "ada@example.com"),
            ("GIT_COMMITTER_NAME", "Ada Lovelace"),
            ("GIT_COMMITTER_EMAIL", "ada@example.com"),
        ]);
        gitcli::output(repo, &args(&["commit", "-S", "--allow-empty", "-m", "x"]), "", &env, Some(Duration::from_secs(60)))
    }

    #[test]
    fn ssh_signing_asks_for_the_key_passphrase() {
        let Some((repo, public)) = ssh_scratch("ssh-sign") else { return };
        let private = public.with_extension("").to_str().unwrap().to_owned();
        let wait = Duration::from_secs(10);

        let (s, seen) = listener(vec![Some("right")]);
        let out = ssh_commit(&repo, &s, &public);
        if !out.ok && out.stderr.contains("usage:") {
            eprintln!("ssh-keygen can't sign (no -Y), skipping: {}", out.stderr);
            let _ = std::fs::remove_dir_all(repo.parent().unwrap());
            return;
        }
        assert!(out.ok, "{}", out.combined());
        assert_eq!(seen.recv_timeout(wait).unwrap(), PromptKind::SshSigningPassphrase { key: private.clone() });
        let (ok, body) = gitcli::run(&repo, &args(&["cat-file", "commit", "HEAD"]));
        assert!(ok && body.contains("-----BEGIN SSH SIGNATURE-----"), "{body}");

        let (s, seen) = listener(vec![None]);
        let out = ssh_commit(&repo, &s, &public);
        assert!(!out.ok, "cancelled: {}", out.combined());
        assert_eq!(seen.recv_timeout(wait).unwrap(), PromptKind::SshSigningPassphrase { key: private });

        let (s, _seen) = listener(vec![Some("wrong")]);
        let out = ssh_commit(&repo, &s, &public);
        assert!(!out.ok, "wrong passphrase: {}", out.combined());
        assert!(glowfly_core::askpass::ssh_sign::wrong_passphrase(&out.combined()), "{}", out.combined());
        let _ = std::fs::remove_dir_all(repo.parent().unwrap());
    }
}
