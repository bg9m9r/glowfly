//! Commit-signing keys: listing GPG secret keys and SSH public keys, generating a GPG key, and
//! exporting public keys. Everything shells out to `gpg` / reads files; no key material is
//! stored by Trellis.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `gpg.format` values git understands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SigningFormat {
    OpenPgp,
    Ssh,
    X509,
}

impl SigningFormat {
    pub const ALL: [SigningFormat; 3] = [SigningFormat::OpenPgp, SigningFormat::Ssh, SigningFormat::X509];

    pub fn config_value(self) -> &'static str {
        match self {
            SigningFormat::OpenPgp => "openpgp",
            SigningFormat::Ssh => "ssh",
            SigningFormat::X509 => "x509",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SigningFormat::OpenPgp => "OpenPGP",
            SigningFormat::Ssh => "SSH",
            SigningFormat::X509 => "X.509",
        }
    }

    pub fn from_config(value: &str) -> SigningFormat {
        match value.trim().to_ascii_lowercase().as_str() {
            "ssh" => SigningFormat::Ssh,
            "x509" => SigningFormat::X509,
            _ => SigningFormat::OpenPgp,
        }
    }

    /// The git config key holding the program for this format.
    pub fn program_key(self) -> &'static str {
        match self {
            SigningFormat::OpenPgp => "gpg.program",
            SigningFormat::Ssh => "gpg.ssh.program",
            SigningFormat::X509 => "gpg.x509.program",
        }
    }

    pub fn default_program(self) -> &'static str {
        match self {
            SigningFormat::OpenPgp => "gpg",
            SigningFormat::Ssh => "ssh-keygen",
            SigningFormat::X509 => "gpgsm",
        }
    }
}

/// A key that can be used as `user.signingkey`.
#[derive(Clone, Debug, PartialEq)]
pub struct SigningKey {
    /// What goes into `user.signingkey`: a long GPG key id, or an SSH public key path.
    pub id: String,
    /// Owner (GPG user id) or comment (SSH).
    pub label: String,
    /// Unix time the key expires, if it does.
    pub expires: Option<i64>,
    pub expired: bool,
}

impl SigningKey {
    pub fn describe(&self) -> String {
        let mut s = if self.label.is_empty() { self.id.clone() } else { format!("{}  {}", self.id, self.label) };
        if self.expired {
            s.push_str("  (expired)");
        }
        s
    }
}

fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run(mut cmd: Command, what: &str) -> Result<String> {
    let out = cmd.output().map_err(|e| Error::Other(format!("could not run {what}: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(Error::Other(format!("{what} failed: {err}")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parses `gpg --list-secret-keys --with-colons` output into signing-capable keys.
pub fn parse_gpg_colons(text: &str, now: i64) -> Vec<SigningKey> {
    let mut keys: Vec<SigningKey> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first().copied() {
            Some("sec") if f.len() > 11 => {
                // Field 12 lists capabilities; an uppercase S means the key can sign.
                if !f[11].contains('S') && !f[11].contains('s') {
                    continue;
                }
                let expires = f[6].parse::<i64>().ok().filter(|t| *t > 0);
                let revoked = f[1] == "r";
                keys.push(SigningKey {
                    id: f[4].to_owned(),
                    label: String::new(),
                    expires,
                    expired: f[1] == "e" || revoked || expires.is_some_and(|t| t <= now),
                });
            }
            Some("uid") if f.len() > 9 => {
                if let Some(k) = keys.last_mut().filter(|k| k.label.is_empty()) {
                    k.label = f[9].replace("\\x3a", ":");
                }
            }
            _ => {}
        }
    }
    keys
}

pub fn list_gpg_keys(program: &str, now: i64) -> Result<Vec<SigningKey>> {
    let mut cmd = command(program);
    cmd.args(["--list-secret-keys", "--with-colons", "--keyid-format=long"]);
    Ok(parse_gpg_colons(&run(cmd, program)?, now))
}

/// SSH public keys in `~/.ssh` (the path is what git expects in `user.signingkey`).
pub fn list_ssh_keys(home: &Path) -> Vec<SigningKey> {
    let dir = home.join(".ssh");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut keys: Vec<SigningKey> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pub"))
        .map(|p| {
            let comment = std::fs::read_to_string(&p).ok().and_then(|t| t.split_whitespace().nth(2).map(str::to_owned)).unwrap_or_default();
            SigningKey { id: p.display().to_string(), label: comment, expires: None, expired: false }
        })
        .collect();
    keys.sort_by(|a, b| a.id.cmp(&b.id));
    keys
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

/// Parameters for `gpg --batch --generate-key`: RSA 4096, expiring in two years.
pub fn gpg_key_params(name: &str, email: &str, passphrase: &str) -> String {
    let protection = if passphrase.is_empty() { "%no-protection".to_owned() } else { format!("Passphrase: {passphrase}") };
    format!(
        "Key-Type: RSA\nKey-Length: 4096\nKey-Usage: sign\nName-Real: {name}\nName-Email: {email}\nExpire-Date: 2y\n{protection}\n%commit\n"
    )
}

/// Generates a signing key and returns its long id.
pub fn generate_gpg_key(program: &str, name: &str, email: &str, passphrase: &str, now: i64) -> Result<String> {
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err(Error::Other("set your name and email in Profile first".into()));
    }
    let mut cmd = command(program);
    cmd.args(["--batch", "--pinentry-mode", "loopback", "--generate-key"]).stdin(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Error::Other(format!("could not run {program}: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(gpg_key_params(name, email, passphrase).as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(Error::Other(format!("key generation failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    // The new key is the one for this email that wasn't there before; pick the newest.
    let keys = list_gpg_keys(program, now)?;
    keys.into_iter()
        .filter(|k| k.label.contains(email.trim()))
        .max_by_key(|k| k.expires.unwrap_or(0))
        .map(|k| k.id)
        .ok_or_else(|| Error::Other("the new key was not found".into()))
}

/// The ASCII-armored public key (GPG) or the `.pub` file contents (SSH).
pub fn export_public_key(format: SigningFormat, program: &str, key: &str) -> Result<String> {
    match format {
        SigningFormat::Ssh => Ok(std::fs::read_to_string(key)?.trim().to_owned()),
        _ => {
            let mut cmd = command(program);
            cmd.args(["--armor", "--export", key]);
            let text = run(cmd, program)?;
            if text.trim().is_empty() { Err(Error::NotFound(format!("no public key for {key}"))) } else { Ok(text) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "sec:u:4096:1:ABCDEF0123456789:1700000000:1800000000::u:::scESC:::+:::23::0:\n\
fpr:::::::::0123456789ABCDEF0123456789ABCDEF01234567:\n\
uid:u::::1700000000::HASH::Ada Lovelace <ada@example.com>::::::::::0:\n\
sec:e:2048:1:1111222233334444:1500000000:1600000000::u:::scESC:::+:::23::0:\n\
uid:e::::1500000000::HASH::Old Key <old@example.com>::::::::::0:\n\
sec:u:255:22:5555666677778888:1700000000:::u:::eE:::+:::ed25519::0:\n\
uid:u::::1700000000::HASH::Encrypt Only <enc@example.com>::::::::::0:\n";

    #[test]
    fn parses_signing_keys() {
        let keys = parse_gpg_colons(SAMPLE, 1_750_000_000);
        assert_eq!(keys.len(), 2, "encrypt-only keys are skipped");
        assert_eq!(keys[0].id, "ABCDEF0123456789");
        assert_eq!(keys[0].label, "Ada Lovelace <ada@example.com>");
        assert_eq!(keys[0].expires, Some(1_800_000_000));
        assert!(!keys[0].expired);
        assert!(keys[1].expired);
        assert!(keys[1].describe().ends_with("(expired)"));
        // Expiry is judged against the given time.
        assert!(parse_gpg_colons(SAMPLE, 1_900_000_000)[0].expired);
    }

    #[test]
    fn formats_and_params() {
        assert_eq!(SigningFormat::from_config("SSH"), SigningFormat::Ssh);
        assert_eq!(SigningFormat::from_config(""), SigningFormat::OpenPgp);
        assert_eq!(SigningFormat::Ssh.program_key(), "gpg.ssh.program");
        let p = gpg_key_params("Ada", "ada@example.com", "");
        assert!(p.contains("Key-Length: 4096") && p.contains("%no-protection") && p.contains("Expire-Date: 2y"));
        assert!(gpg_key_params("Ada", "a@b", "secret").contains("Passphrase: secret"));
    }

    #[test]
    fn lists_ssh_public_keys() {
        let home = std::env::temp_dir().join(format!("trellis-ssh-{}", std::process::id()));
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("id_ed25519.pub"), "ssh-ed25519 AAAAC3Nz ada@laptop\n").unwrap();
        std::fs::write(ssh.join("id_ed25519"), "private").unwrap();
        let keys = list_ssh_keys(&home);
        assert_eq!(keys.len(), 1);
        assert!(keys[0].id.ends_with("id_ed25519.pub"));
        assert_eq!(keys[0].label, "ada@laptop");
        let _ = std::fs::remove_dir_all(&home);
    }
}
