//! Signing passphrases: Glowfly as git's `gpg.program`.
//!
//! git signs commits and tags by running its OpenPGP program as `<program> --status-fd=2 -bsau
//! <key>`, writing the payload on stdin, reading the signature from stdout and checking stderr
//! for `[GNUPG:] SIG_CREATED`. When gpg-agent needs the key's passphrase it starts the pinentry
//! it was configured with, shared by every client of the long-running agent, so Glowfly can't
//! swap in its own per call. GnuPG's per-call alternative is `--pinentry-mode loopback`: gpg
//! reads the passphrase itself and hands it to the agent.
//!
//! For git commands that may prompt, `gitcli` sets `gpg.program` to the Glowfly executable
//! through command-scope config ([`env::vars`]), which wins over every config file. Run that
//! way ([`is_wrapper_call`]), Glowfly:
//!
//! 1. finds the program the user configured ([`program::user_program`]; `gpg` by default),
//!    mapping MSYS-style paths (`/usr/bin/gpg`) into the Git for Windows install on Windows;
//! 2. tries it with `--pinentry-mode error` ([`args::probe`]): a cached or unprotected key
//!    signs straight away and nothing is shown;
//! 3. on "No pinentry", asks the app through the askpass listener ([`crate::askpass`]) with a
//!    structured request ([`crate::askpass::GpgAsk`]), and signs with loopback pinentry, the
//!    passphrase fed on fd 0 ahead of the payload ([`args::loopback`]); a bad passphrase asks
//!    again, up to [`MAX_TRIES`] times;
//! 4. otherwise (the app says to use GnuPG's own window, can't be reached, or GnuPG doesn't
//!    support loopback) runs the program exactly as git asked.
//!
//! Anything that isn't a sign call (signature verification) passes straight through.
//! [`flow`] is the state machine, generic over the gpg runs and the asking so tests use fakes;
//! [`real`] runs the real program and asks the real app.
//!
//! With `gpg.format=ssh` git runs `gpg.ssh.program` (ssh-keygen) itself, not the wrapper.
//! ssh-keygen asks for a protected key's passphrase through `SSH_ASKPASS`, which gitcli already
//! sets, and gitcli adds `GLOWFLY_SSH_SIGNING_KEY` for commands that may sign, so the helper
//! sends an [`crate::askpass::SshSignAsk`] ([`ssh_env`], [`crate::askpass::ssh_sign`]).

pub mod args;
pub mod env;
pub mod extend;
pub mod flow;
pub mod program;
pub mod real;
pub mod ssh_env;
pub mod status;

#[cfg(test)]
mod args_tests;
#[cfg(test)]
mod env_tests;
#[cfg(test)]
mod extend_tests;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod ssh_env_tests;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;

use std::ffi::{OsStr, OsString};

/// Marks a process git started as the wrapper (`1`), and the wrapper's own gpg children
/// (`nested`), so a `gpg.program` that leads back to Glowfly stops instead of looping.
pub const ENV_WRAPPER: &str = "GLOWFLY_GPG_WRAPPER";

/// The git program Glowfly ran (the one chosen in Preferences > Git, or `git`), so the wrapper
/// reads `gpg.program` with the same git that started it.
pub const ENV_GIT: &str = "GLOWFLY_GIT_PROGRAM";

/// How many passphrases are tried before git is told signing failed.
pub const MAX_TRIES: usize = 3;

/// What a nested call prints before exiting 2.
pub const LOOP_MESSAGE: &str = "Glowfly: gpg.program points back at Glowfly";

/// Whether this process was started as git's gpg.program: the marker is set and the first
/// argument is an option. Askpass prompt texts never start with `-`, and the other command-line
/// modes are excluded by name.
pub fn is_wrapper_call(argv: &[OsString], marker: Option<&OsStr>) -> bool {
    let marked = marker.is_some_and(|m| m == "1" || m == "nested");
    marked && argv.get(1).is_some_and(|a| a.to_string_lossy().starts_with('-') && a != "--askpass" && a != "--write-todo")
}

/// Whether the marker says this process is the wrapper's own gpg child: `gpg.program` resolved
/// back to Glowfly.
pub fn is_nested(marker: Option<&OsStr>) -> bool {
    marker.is_some_and(|m| m == "nested")
}

/// The wrapper's entry point: `args` are git's arguments for gpg. Returns the exit code.
pub fn run_wrapper(args: &[OsString]) -> i32 {
    if is_nested(std::env::var_os(ENV_WRAPPER).as_deref()) {
        eprintln!("{LOOP_MESSAGE}");
        return 2;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let mut real = real::Real::new(program::user_program(&exe));
    flow::run(args, &mut real)
}
