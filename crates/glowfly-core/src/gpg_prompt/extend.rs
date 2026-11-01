//! Extending an expired OpenPGP key from Preferences > Commit signing: `gpg --quick-set-expire`
//! on the primary key and on each of its subkeys that isn't revoked, two years from now.
//!
//! The passphrase is asked the way the signing wrapper asks it ([`super::flow`]): a first run
//! with `--pinentry-mode error`, so an unprotected or cached key needs nothing; on "No
//! pinentry", the app's own prompt (through the askpass listener, [`InApp`]) and a run with
//! loopback pinentry, the passphrase on fd 0; a bad passphrase asks again, up to
//! [`super::MAX_TRIES`] times. When the app says to use GnuPG's own window (the preference is
//! off), can't be reached, or GnuPG can't do loopback, gpg runs without a pinentry mode, so its
//! own pinentry asks. [`extend`] is generic over the gpg runs and the asking, so tests use fakes.

use super::MAX_TRIES;
use super::flow::{AskResult, Asker, Gpg, GpgRun};
use super::status::{SignResult, classify};
use crate::askpass::{self, GpgAsk, Outcome};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::Stdio;

/// How far from now the new expiry is (gpg's notation).
pub const EXTEND_BY: &str = "2y";

/// Why extending stopped.
#[derive(Clone, Debug, PartialEq)]
pub enum ExtendFailed {
    Cancelled,
    TimedOut,
    /// gpg's own message (without its status lines), or why it couldn't run.
    Gpg(String),
}

/// The fingerprints `--quick-set-expire` takes: the first secret key's, and those of its
/// subkeys that aren't revoked.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyPrints {
    pub primary: String,
    pub subkeys: Vec<String>,
    /// The key's first user id, for the passphrase prompt.
    pub user: Option<String>,
}

/// Reads [`KeyPrints`] from `gpg --list-secret-keys --with-colons` output for one key. Each
/// `fpr` line follows the `sec` or `ssb` line it belongs to.
pub fn key_prints(colons: &str) -> Option<KeyPrints> {
    let (mut primary, mut subkeys, mut user) = (None::<String>, Vec::new(), None);
    // What the next `fpr` line is for: the primary key, a subkey to extend, or neither.
    let mut next: Option<bool> = None;
    for line in colons.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first().copied() {
            Some("sec") if primary.is_some() => break,
            Some("sec") => next = Some(true),
            Some("ssb") => next = (f.get(1) != Some(&"r")).then_some(false),
            Some("fpr") => {
                let print = f.get(9).filter(|p| !p.is_empty()).map(|p| (*p).to_owned());
                match (next.take(), print) {
                    (Some(true), Some(p)) => primary = Some(p),
                    (Some(false), Some(p)) => subkeys.push(p),
                    _ => {}
                }
            }
            Some("uid") if user.is_none() => user = f.get(9).filter(|u| !u.is_empty()).map(|u| u.replace("\\x3a", ":")),
            _ => {}
        }
    }
    Some(KeyPrints { primary: primary?, subkeys, user })
}

/// How a run asks for the passphrase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Fail rather than start a pinentry.
    Probe,
    /// The passphrase on stdin's first line.
    Loopback,
    /// GnuPG's own pinentry.
    System,
}

/// The arguments of one `--quick-set-expire` run: the primary key, or (`subkeys`) the given
/// subkeys of it.
pub fn set_expire_args(prints: &KeyPrints, subkeys: bool, mode: Mode) -> Vec<OsString> {
    let pinentry: &[&str] = match mode {
        Mode::Probe => &["--batch", "--pinentry-mode", "error"],
        Mode::Loopback => &["--batch", "--pinentry-mode", "loopback", "--passphrase-fd", "0"],
        Mode::System => &[],
    };
    let mut args: Vec<OsString> = pinentry.iter().map(OsString::from).collect();
    args.extend(["--status-fd", "2", "--quick-set-expire", &prints.primary, EXTEND_BY].map(OsString::from));
    if subkeys {
        args.extend(prints.subkeys.iter().map(OsString::from));
    }
    args
}

/// How one run ended.
fn outcome(run: &GpgRun) -> SignResult {
    if run.ok {
        return SignResult::Created;
    }
    let stderr = String::from_utf8_lossy(&run.stderr);
    match classify(false, &stderr) {
        SignResult::Failed => {
            let lower = stderr.to_ascii_lowercase();
            if lower.contains("no pinentry") {
                SignResult::NeedsPassphrase
            } else if lower.contains("bad passphrase") {
                SignResult::BadPassphrase
            } else {
                SignResult::Failed
            }
        }
        other => other,
    }
}

/// gpg's message: its stderr without the `[GNUPG:]` status lines.
fn message(run: &GpgRun) -> String {
    let text = String::from_utf8_lossy(&run.stderr);
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.is_empty() && !l.starts_with("[GNUPG:]")).collect();
    let joined = lines.join("\n");
    if joined.is_empty() { "gpg failed without saying why".to_owned() } else { joined }
}

/// Runs the primary key's and then the subkeys' change in `mode`, feeding `input` to each.
fn both(prints: &KeyPrints, mode: Mode, input: &[u8], gpg: &mut impl Gpg) -> GpgRun {
    let first = attempt(gpg, &set_expire_args(prints, false, mode), input);
    if !first.ok || prints.subkeys.is_empty() {
        return first;
    }
    attempt(gpg, &set_expire_args(prints, true, mode), input)
}

fn attempt(gpg: &mut impl Gpg, args: &[OsString], input: &[u8]) -> GpgRun {
    gpg.run(args, input)
        .unwrap_or_else(|e| GpgRun { stderr: format!("could not run {}: {e}\n", gpg.program()).into_bytes(), ..GpgRun::default() })
}

fn done(run: &GpgRun) -> Result<(), ExtendFailed> {
    if run.ok { Ok(()) } else { Err(ExtendFailed::Gpg(message(run))) }
}

/// Moves the expiry of the key `prints` names (and its subkeys) to two years from now, asking
/// for its passphrase when the agent needs one.
pub fn extend(prints: &KeyPrints, gpg: &mut impl Gpg, asker: &mut impl Asker) -> Result<(), ExtendFailed> {
    let probe = both(prints, Mode::Probe, &[], gpg);
    match outcome(&probe) {
        SignResult::Created => return Ok(()),
        SignResult::NeedsPassphrase => {}
        SignResult::Unsupported => return done(&both(prints, Mode::System, &[], gpg)),
        _ => return done(&probe),
    }
    let mut last = probe;
    for attempt_no in 1..=MAX_TRIES {
        let ask = GpgAsk { key: prints.primary.clone(), user: prints.user.clone(), retry: attempt_no > 1 };
        let secret = match asker.ask(&ask) {
            AskResult::Answer(secret) => secret,
            AskResult::Cancelled => return Err(ExtendFailed::Cancelled),
            AskResult::TimedOut => return Err(ExtendFailed::TimedOut),
            AskResult::UseSystem | AskResult::Unavailable => return done(&both(prints, Mode::System, &[], gpg)),
        };
        let mut input = format!("{}\n", secret.expose()).into_bytes();
        let run = both(prints, Mode::Loopback, &input, gpg);
        input.iter_mut().for_each(|b| *b = 0);
        match outcome(&run) {
            SignResult::Created => return Ok(()),
            SignResult::BadPassphrase => last = run,
            SignResult::Unsupported => return done(&both(prints, Mode::System, &[], gpg)),
            _ => return done(&run),
        }
    }
    done(&last)
}

/// The user's OpenPGP program, run as the Preferences page runs it (`signing::command`).
pub struct Direct {
    program: String,
}

impl Gpg for Direct {
    fn run(&mut self, args: &[OsString], input: &[u8]) -> io::Result<GpgRun> {
        let mut cmd = crate::signing::command(&self.program);
        cmd.args(args).stdin(Stdio::piped());
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            // gpg may exit without reading it (a failure); that's fine.
            let _ = stdin.write_all(input);
        }
        let out = child.wait_with_output()?;
        Ok(GpgRun { ok: out.status.success(), code: out.status.code(), stdout: out.stdout, stderr: out.stderr })
    }

    fn describe(&mut self, key: Option<&str>) -> (String, Option<String>) {
        (key.unwrap_or_default().to_owned(), None)
    }

    fn program(&self) -> &str {
        &self.program
    }
}

/// Asks this app's own prompt, through the askpass listener every git child uses.
pub struct InApp;

impl Asker for InApp {
    fn ask(&mut self, ask: &GpgAsk) -> AskResult {
        match askpass::ask_gpg_in_app(ask) {
            Ok(Outcome::Answer(s)) => AskResult::Answer(s),
            Ok(Outcome::Cancelled) => AskResult::Cancelled,
            Ok(Outcome::TimedOut) => AskResult::TimedOut,
            Ok(Outcome::UseSystem) => AskResult::UseSystem,
            Err(_) => AskResult::Unavailable,
        }
    }
}

/// Extends the secret key `key` (a key id or fingerprint) with `program`.
pub fn extend_gpg_key(program: &str, key: &str) -> Result<(), ExtendFailed> {
    let mut gpg = Direct { program: program.to_owned() };
    let list = ["--batch", "--list-secret-keys", "--with-colons", "--keyid-format=long", "--", key].map(OsString::from);
    let listed = attempt(&mut gpg, &list, &[]);
    if !listed.ok {
        return Err(ExtendFailed::Gpg(message(&listed)));
    }
    let Some(prints) = key_prints(&String::from_utf8_lossy(&listed.stdout)) else {
        return Err(ExtendFailed::Gpg(format!("no secret key {key}")));
    };
    extend(&prints, &mut gpg, &mut InApp)
}
