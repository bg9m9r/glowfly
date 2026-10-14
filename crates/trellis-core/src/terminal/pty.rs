//! Shell sessions on a pseudo-terminal: which shell to start, and a running session with its
//! output and exit reported through callbacks on their own threads.

use crate::error::{Error, Result};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;

/// A program and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// The shell to start. Windows: PowerShell 7 (pwsh) if it is on PATH, else Windows PowerShell,
/// else COMSPEC (cmd.exe). Elsewhere: $SHELL, else /bin/sh.
pub fn choose_shell(windows: bool, env: impl Fn(&str) -> Option<String>, on_path: impl Fn(&str) -> bool) -> ShellCommand {
    let nologo = |program: &str| ShellCommand { program: program.into(), args: vec!["-NoLogo".into()] };
    if windows {
        if on_path("pwsh.exe") {
            nologo("pwsh.exe")
        } else if on_path("powershell.exe") {
            nologo("powershell.exe")
        } else {
            let program = env("COMSPEC").filter(|s| !s.is_empty()).unwrap_or_else(|| "cmd.exe".into());
            ShellCommand { program, args: Vec::new() }
        }
    } else {
        let program = env("SHELL").filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
        ShellCommand { program, args: Vec::new() }
    }
}

/// `choose_shell` for this system, from the environment and PATH.
pub fn default_shell() -> ShellCommand {
    let on_path =
        |name: &str| std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()));
    choose_shell(cfg!(windows), |k| std::env::var(k).ok(), on_path)
}

/// A running shell. Dropping it closes the terminal. On Windows the output reader only ends
/// once this is dropped, so the owner drops it after the exit callback has run.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl PtySession {
    /// Sends input to the program.
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(size(rows, cols)).map_err(other)
    }

    /// Stops the shell (and, on Windows, everything attached to its console).
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.killer.kill()
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

/// portable-pty reports errors as `anyhow::Error`; `{:#}` keeps its whole cause chain.
fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("{e:#}"))
}

/// Starts `shell` in `cwd` on a new pseudo-terminal of `rows` × `cols`. `on_output` gets each
/// chunk the program writes (on a reader thread) and `on_exit` its exit code once it ends
/// (on a waiter thread; None if waiting failed).
pub fn spawn(
    cwd: &Path,
    rows: u16,
    cols: u16,
    shell: &ShellCommand,
    mut on_output: impl FnMut(Vec<u8>) + Send + 'static,
    on_exit: impl FnOnce(Option<u32>) + Send + 'static,
) -> Result<PtySession> {
    let pair = native_pty_system().openpty(size(rows, cols)).map_err(other)?;
    let mut cmd = CommandBuilder::new(&shell.program);
    cmd.args(&shell.args);
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd).map_err(other)?;
    // The pseudoconsole only reports the end of output once nothing else holds the slave.
    drop(pair.slave);
    let killer = child.clone_killer();
    let mut reader = pair.master.try_clone_reader().map_err(other)?;
    let writer = pair.master.take_writer().map_err(other)?;
    std::thread::Builder::new().name("trellis-pty-read".into()).spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => on_output(buf[..n].to_vec()),
            }
        }
    })?;
    std::thread::Builder::new().name("trellis-pty-wait".into()).spawn(move || {
        on_exit(child.wait().ok().map(|s| s.exit_code()));
    })?;
    Ok(PtySession { master: pair.master, writer, killer })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn chooses_a_shell() {
        let env =
            |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        let path = |names: &'static [&'static str]| move |n: &str| names.contains(&n);
        let nologo = vec!["-NoLogo".to_owned()];

        let s = choose_shell(true, env(&[]), path(&["pwsh.exe", "powershell.exe"]));
        assert_eq!((s.program.as_str(), &s.args), ("pwsh.exe", &nologo));
        let s = choose_shell(true, env(&[]), path(&["powershell.exe"]));
        assert_eq!((s.program.as_str(), &s.args), ("powershell.exe", &nologo));
        let s = choose_shell(true, env(&[("COMSPEC", r"C:\Windows\system32\cmd.exe")]), path(&[]));
        assert_eq!(s, ShellCommand { program: r"C:\Windows\system32\cmd.exe".into(), args: vec![] });
        assert_eq!(choose_shell(true, env(&[]), path(&[])).program, "cmd.exe");

        let s = choose_shell(false, env(&[("SHELL", "/bin/zsh")]), path(&["pwsh.exe"]));
        assert_eq!(s, ShellCommand { program: "/bin/zsh".into(), args: vec![] });
        assert_eq!(choose_shell(false, env(&[]), path(&[])).program, "/bin/sh");
        assert_eq!(choose_shell(false, env(&[("SHELL", "")]), path(&[])).program, "/bin/sh");
    }

    #[test]
    fn spawns_a_command_and_reports_output_and_exit() {
        let dir = std::env::temp_dir().join(format!("trellis-pty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shell = if cfg!(windows) {
            ShellCommand { program: "cmd.exe".into(), args: vec!["/C".into(), "echo trellis-pty".into()] }
        } else {
            ShellCommand { program: "/bin/sh".into(), args: vec!["-c".into(), "printf trellis-pty".into()] }
        };
        let (out_tx, out_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let mut session = spawn(
            &dir,
            24,
            80,
            &shell,
            move |b| {
                let _ = out_tx.send(b);
            },
            move |code| {
                let _ = exit_tx.send(code);
            },
        )
        .expect("spawn");
        let deadline = Instant::now() + Duration::from_secs(20);
        // The screen answers the pseudoconsole's cursor position query, as the app does.
        let mut screen = crate::terminal::TermScreen::new(24, 80);
        let mut text = Vec::new();
        while !String::from_utf8_lossy(&text).contains("trellis-pty") {
            let left = deadline.saturating_duration_since(Instant::now());
            match out_rx.recv_timeout(left) {
                Ok(b) => {
                    let reply = screen.feed(&b);
                    if !reply.is_empty() {
                        session.write(&reply).expect("reply");
                    }
                    text.extend(b);
                }
                Err(e) => panic!("no output ({e}): {:?}", String::from_utf8_lossy(&text)),
            }
        }
        let code = exit_rx.recv_timeout(Duration::from_secs(20)).expect("exit");
        assert_eq!(code, Some(0));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
