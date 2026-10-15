//! Checking commit signatures with git (`git log --format=%G?…`), so the user's gpg/ssh/gpgsm
//! programs, keyrings, trust and gpg.ssh.allowedSignersFile apply.
//!
//! Whether a commit is signed at all comes from its header ([`signature_format`], libgit2, no
//! program runs); only a signed commit is checked ([`verify_commit`]).

use crate::error::{Error, Result};
use crate::gitcli;
use crate::signing::SigningFormat;
use crate::text::printable;
use git2::Oid;
use std::path::Path;

#[cfg(test)]
mod tests;

/// Placeholders: status, signer, key, fingerprint, trust level, raw output, NUL-separated.
pub const LOG_FORMAT: &str = "--format=%G?%x00%GS%x00%GK%x00%GF%x00%GT%x00%GG";

/// What git's check found (its `%G?` letter, with N split by cause).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigStatus {
    /// G: a good signature from a trusted key (SSH: one the allowed signers file lists).
    Good,
    /// U: a good signature, but the key's validity is unknown or untrusted (SSH: no principal
    /// matched).
    GoodUntrusted,
    /// X: the signature has expired.
    ExpiredSignature,
    /// Y: the key that made it has expired.
    ExpiredKey,
    /// R: the key that made it has been revoked.
    RevokedKey,
    /// B: the signature doesn't match the commit.
    Bad,
    /// E: the signature can't be checked (e.g. the public key is missing).
    CannotCheck,
    /// N for a signed commit, with git's complaint that no allowed signers file is set.
    NoAllowedSigners,
    /// N (or an unknown letter) for a signed commit: git couldn't check it.
    NotChecked,
}

/// The badge a check earns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    Signed,
    Expired,
    Revoked,
    Bad,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Verified => "Verified",
            Verdict::Signed => "Signed",
            Verdict::Expired => "Expired",
            Verdict::Revoked => "Revoked",
            Verdict::Bad => "Bad signature",
        }
    }
}

/// The result of checking one signed commit.
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureCheck {
    pub format: SigningFormat,
    pub status: SigStatus,
    /// %GS: the GPG user ID, or the SSH principal (printable).
    pub signer: String,
    /// %GK: the key that made the signature.
    pub key: String,
    /// %GF: the key's fingerprint (for SSH the same as `key`).
    pub fingerprint: String,
    /// %GT when it is one of never / marginal / fully / ultimate.
    pub trust: Option<String>,
    /// First meaningful line from git's stderr (an "error: " prefix stripped) or, failing that,
    /// %GG's last line, for failure reasons. Empty for Good.
    pub note: String,
}

impl SignatureCheck {
    pub fn verdict(&self) -> Verdict {
        match self.status {
            SigStatus::Good => Verdict::Verified,
            SigStatus::GoodUntrusted | SigStatus::CannotCheck | SigStatus::NoAllowedSigners | SigStatus::NotChecked => Verdict::Signed,
            SigStatus::ExpiredSignature | SigStatus::ExpiredKey => Verdict::Expired,
            SigStatus::RevokedKey => Verdict::Revoked,
            SigStatus::Bad => Verdict::Bad,
        }
    }

    /// One or two sentences saying what the result means, for the badge's tooltip.
    pub fn reason(&self) -> String {
        let ssh = self.format == SigningFormat::Ssh;
        match self.status {
            SigStatus::Good if ssh => "Good signature from a key your allowed signers file lists for this signer.".into(),
            SigStatus::Good => "Good signature from a key you trust.".into(),
            SigStatus::GoodUntrusted if ssh => "The signature is good, but your allowed signers file doesn't list this key for this \
                                                signer, so who made it isn't confirmed."
                .into(),
            SigStatus::GoodUntrusted => {
                "The signature is good, but you haven't marked this key as trusted in GPG, so who made it isn't confirmed.".into()
            }
            SigStatus::CannotCheck => {
                let missing_key = self.note.is_empty() || self.note.to_ascii_lowercase().contains("public key");
                if self.format == SigningFormat::OpenPgp && missing_key {
                    "The signer's public key isn't in your keyring, so the signature can't be checked.".into()
                } else {
                    couldnt_check(&self.note)
                }
            }
            SigStatus::NoAllowedSigners => "SSH signatures are checked against an allowed signers file, and none is set \
                                            (gpg.ssh.allowedSignersFile). Choose one in Preferences > Commit Signing."
                .into(),
            SigStatus::NotChecked => couldnt_check(&self.note),
            SigStatus::ExpiredSignature => "The signature has expired.".into(),
            SigStatus::ExpiredKey => "The key that made the signature has expired.".into(),
            SigStatus::RevokedKey => "The key that made the signature has been revoked.".into(),
            SigStatus::Bad => {
                "The signature doesn't match this commit: the commit changed after it was signed, or the signature is damaged.".into()
            }
        }
    }

    /// The signer, key, fingerprint (when it isn't the key) and trust level (OpenPGP), as
    /// (label, value); empty ones are left out.
    pub fn details(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        if !self.signer.is_empty() {
            out.push(("Signer", self.signer.clone()));
        }
        if !self.key.is_empty() {
            out.push(("Key", self.key.clone()));
        }
        if !self.fingerprint.is_empty() && self.fingerprint != self.key {
            out.push(("Fingerprint", self.fingerprint.clone()));
        }
        if self.format == SigningFormat::OpenPgp
            && let Some(t) = &self.trust
        {
            out.push(("Trust", t.clone()));
        }
        out
    }
}

fn couldnt_check(note: &str) -> String {
    if note.is_empty() { "Git couldn't check the signature.".into() } else { format!("Git couldn't check the signature: {note}") }
}

/// The format of the signature in commit `oid`'s header, or None if it is unsigned. libgit2
/// reads only the header (`Repository::extract_signature(&oid, None)`). No program runs.
pub fn signature_format(repo: &git2::Repository, oid: Oid) -> Option<SigningFormat> {
    repo.extract_signature(&oid, None).ok().map(|(sig, _)| format_of(&sig))
}

/// The format an armored signature is in: "-----BEGIN SSH SIGNATURE" → Ssh,
/// "-----BEGIN SIGNED MESSAGE" → X509, anything else (PGP SIGNATURE / PGP MESSAGE) → OpenPgp.
pub fn format_of(armored: &[u8]) -> SigningFormat {
    let start = armored.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(armored.len());
    let text = &armored[start..];
    if text.starts_with(b"-----BEGIN SSH SIGNATURE") {
        SigningFormat::Ssh
    } else if text.starts_with(b"-----BEGIN SIGNED MESSAGE") {
        SigningFormat::X509
    } else {
        SigningFormat::OpenPgp
    }
}

/// Parses [`LOG_FORMAT`] output for a commit signed in `format`. Err when stdout has no status
/// letter.
pub fn parse_check(format: SigningFormat, stdout: &str, stderr: &str) -> Result<SignatureCheck> {
    let out = stdout.strip_suffix('\n').unwrap_or(stdout);
    let mut fields = out.splitn(6, '\0');
    let letter = fields.next().and_then(|f| f.chars().next()).ok_or_else(|| Error::Other("git printed no signature status".into()))?;
    let mut next = || fields.next().unwrap_or_default();
    let (signer, key, fingerprint, trust, raw) = (next(), next(), next(), next(), next());
    let status = match letter {
        'G' => SigStatus::Good,
        'U' => SigStatus::GoodUntrusted,
        'X' => SigStatus::ExpiredSignature,
        'Y' => SigStatus::ExpiredKey,
        'R' => SigStatus::RevokedKey,
        'B' => SigStatus::Bad,
        'E' => SigStatus::CannotCheck,
        _ if stderr.contains("allowedSignersFile") => SigStatus::NoAllowedSigners,
        _ => SigStatus::NotChecked,
    };
    let trust = matches!(trust, "never" | "marginal" | "fully" | "ultimate").then(|| trust.to_owned());
    let note = if status == SigStatus::Good { String::new() } else { note(stderr, raw) };
    Ok(SignatureCheck { format, status, signer: printable(signer), key: printable(key), fingerprint: printable(fingerprint), trust, note })
}

/// The first non-empty stderr line without git's "error: " / "fatal: " prefix, or else the
/// last non-empty line of %GG.
fn note(stderr: &str, raw: &str) -> String {
    let line = match stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(l) => ["error: ", "fatal: "].iter().find_map(|p| l.strip_prefix(p)).unwrap_or(l),
        None => raw.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or_default(),
    };
    printable(line)
}

/// Runs git's signature check on `oid` (signed, in `format`): `git log -1 --no-show-signature
/// LOG_FORMAT <oid> --`, never prompting and stopped after [`gitcli::READ_TIMEOUT`] (gpg can
/// hang fetching a key).
pub fn verify_commit(workdir: &Path, oid: Oid, format: SigningFormat) -> Result<SignatureCheck> {
    verify_commit_with(workdir, oid, format, &[])
}

/// [`verify_commit`] with extra environment after `NO_PROMPTS`. Tests use it to point
/// GIT_CONFIG_GLOBAL at an empty file and set GIT_CONFIG_NOSYSTEM=1.
pub(crate) fn verify_commit_with(workdir: &Path, oid: Oid, format: SigningFormat, env: &[(&str, &str)]) -> Result<SignatureCheck> {
    let oid = oid.to_string();
    let args = gitcli::args(&["log", "-1", "--no-show-signature", LOG_FORMAT, &oid, "--"]);
    let env: Vec<(&str, &str)> = gitcli::NO_PROMPTS.iter().chain(env).copied().collect();
    let out = gitcli::output(workdir, &args, "", &env, Some(gitcli::READ_TIMEOUT));
    if !out.ok {
        return Err(Error::Other(format!("git log failed: {}", out.stderr.trim())));
    }
    parse_check(format, &out.stdout, &out.stderr)
}
