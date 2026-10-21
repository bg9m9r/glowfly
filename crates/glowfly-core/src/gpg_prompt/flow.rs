//! The wrapper's steps: probe, ask, sign with loopback, or fall back to the program's own
//! pinentry. [`sign`] is generic over running gpg and asking the app, so tests use fakes.

use super::MAX_TRIES;
use super::args::{self, SignCall};
use super::real::{EnvAsker, Real};
use super::status::{SignResult, classify};
use crate::askpass::{ENV_ADDR, ENV_TOKEN, GpgAsk, Secret};
use std::ffi::OsString;
use std::io::{self, Read, Write};

/// One finished gpg run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpgRun {
    pub ok: bool,
    /// None when the program couldn't be run or was killed.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs the user's OpenPGP program.
pub trait Gpg {
    /// Runs it with `args`, feeding `input` on stdin.
    fn run(&mut self, args: &[OsString], input: &[u8]) -> io::Result<GpgRun>;
    /// The key id (long id, else the spec, else "your signing key") and its user id, for the
    /// dialog.
    fn describe(&mut self, key: Option<&str>) -> (String, Option<String>);
    /// The program's name, for error messages.
    fn program(&self) -> &str;
}

/// How asking the app ended.
#[derive(Debug)]
pub enum AskResult {
    Answer(Secret),
    Cancelled,
    TimedOut,
    /// The user wants GnuPG's own window (the preference is off).
    UseSystem,
    /// The app couldn't be reached.
    Unavailable,
}

/// Asks the app for the passphrase.
pub trait Asker {
    fn ask(&mut self, ask: &GpgAsk) -> AskResult;
}

/// How signing ended.
#[derive(Debug, PartialEq)]
pub enum Finish {
    /// Hand this run's output and exit code to git.
    Done(GpgRun),
    Cancelled,
    TimedOut,
}

/// Runs gpg, turning a failure to start it into a failed run.
fn attempt(gpg: &mut impl Gpg, args: &[OsString], input: &[u8]) -> GpgRun {
    gpg.run(args, input)
        .unwrap_or_else(|e| GpgRun { stderr: format!("Glowfly: could not run {}: {e}\n", gpg.program()).into_bytes(), ..GpgRun::default() })
}

fn result(run: &GpgRun) -> SignResult {
    classify(run.ok, &String::from_utf8_lossy(&run.stderr))
}

/// Signs `payload` for `call`, asking for the passphrase when the agent needs one.
pub fn sign(call: &SignCall, payload: &[u8], gpg: &mut impl Gpg, asker: &mut impl Asker) -> Finish {
    let probe = attempt(gpg, &args::probe(call), payload);
    match result(&probe) {
        SignResult::Created => return Finish::Done(probe),
        SignResult::NeedsPassphrase => {}
        _ => return Finish::Done(attempt(gpg, &call.original, payload)),
    }
    let (key, user) = gpg.describe(call.key.as_deref());
    let mut last = probe;
    for attempt_no in 1..=MAX_TRIES {
        let secret = match asker.ask(&GpgAsk { key: key.clone(), user: user.clone(), retry: attempt_no > 1 }) {
            AskResult::Answer(secret) => secret,
            AskResult::Cancelled => return Finish::Cancelled,
            AskResult::TimedOut => return Finish::TimedOut,
            AskResult::UseSystem | AskResult::Unavailable => return Finish::Done(attempt(gpg, &call.original, payload)),
        };
        // gpg reads only the first line of the passphrase fd; the payload follows.
        let mut input = Vec::with_capacity(secret.expose().len() + 1 + payload.len());
        input.extend_from_slice(secret.expose().as_bytes());
        input.push(b'\n');
        input.extend_from_slice(payload);
        let run = attempt(gpg, &args::loopback(call), &input);
        wipe_bytes(&mut input);
        match result(&run) {
            SignResult::Created => return Finish::Done(run),
            SignResult::BadPassphrase => last = run,
            SignResult::Unsupported => return Finish::Done(attempt(gpg, &call.original, payload)),
            _ => return Finish::Done(run),
        }
    }
    Finish::Done(last)
}

/// Overwrites `bytes` (best effort, like [`crate::askpass::wipe`]).
fn wipe_bytes(bytes: &mut Vec<u8>) {
    bytes.iter_mut().for_each(|b| *b = 0);
    std::hint::black_box(&bytes);
    bytes.clear();
}

/// The wrapper's work for git's arguments `args`. Returns the exit code for git.
pub fn run(args: &[OsString], real: &mut Real) -> i32 {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    let Some(call) = args::parse(args).filter(|_| set(ENV_ADDR) && set(ENV_TOKEN)) else {
        return super::real::passthrough(real.program(), args);
    };
    let mut payload = Vec::new();
    if let Err(e) = io::stdin().lock().read_to_end(&mut payload) {
        eprintln!("Glowfly: could not read what git asked to sign: {e}");
        return 2;
    }
    match sign(&call, &payload, real, &mut EnvAsker) {
        Finish::Done(run) => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(&run.stdout).and_then(|()| out.flush());
            let _ = io::stderr().lock().write_all(&run.stderr);
            run.code.unwrap_or(2)
        }
        Finish::Cancelled => {
            eprintln!("Glowfly: signing stopped: the passphrase prompt was cancelled.");
            1
        }
        Finish::TimedOut => {
            eprintln!("Glowfly: signing stopped: the passphrase prompt timed out.");
            1
        }
    }
}
