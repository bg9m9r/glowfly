//! Reading key pairs: public key lines, `ssh-keygen -l` fingerprints, the pairs in a folder, a
//! pair picked anywhere (Use existing) and the key names ssh tries by default.

use super::{SshKeyPair, pub_path};
use crate::error::Result;
use crate::paths::same_spelling;
use crate::signing::command;
use std::path::{Path, PathBuf};

/// The key type and comment of a public key line (the first line of a `.pub` file). Lines
/// that don't start with a key type, such as authorized_keys entries with options, are None.
pub fn parse_public_key(text: &str) -> Option<(String, String)> {
    let (key_type, _) = key_body(text)?;
    let mut tokens = text.lines().next()?.split_whitespace().skip(2);
    let comment = tokens.by_ref().collect::<Vec<_>>().join(" ");
    Some((key_type.to_owned(), comment))
}

/// The key type and base64 body of a public key line, without its comment: two lines name the
/// same key when these match.
pub fn key_body(public_line: &str) -> Option<(&str, &str)> {
    let mut tokens = public_line.lines().next()?.split_whitespace();
    let key_type = tokens.next()?;
    if !["ssh-", "ecdsa-sha2-", "sk-"].iter().any(|p| key_type.starts_with(p)) {
        return None;
    }
    Some((key_type, tokens.next()?))
}

/// The SHA256 fingerprint of a public key line, as `ssh-add -l` and `ssh-keygen -l` print it by
/// default: "SHA256:" and the unpadded base64 of the SHA-256 of the decoded key body. None when
/// the line isn't a public key or its body isn't base64.
pub fn public_fingerprint(public_line: &str) -> Option<String> {
    use base64::Engine;
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
    use sha2::{Digest, Sha256};
    let (_, body) = key_body(public_line)?;
    let blob = STANDARD.decode(body).ok()?;
    Some(format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(&blob))))
}

/// Parses `ssh-keygen -l` output, e.g. "256 SHA256:abc ada@x (ED25519)", into the size, the
/// fingerprint and the kind in the trailing parentheses.
pub fn parse_fingerprint(stdout: &str) -> Option<(Option<u32>, String, String)> {
    let tokens: Vec<&str> = stdout.lines().next()?.split_whitespace().collect();
    if tokens.len() < 3 {
        return None;
    }
    let fingerprint = tokens[1];
    let kind = tokens[tokens.len() - 1].strip_prefix('(')?.strip_suffix(')')?;
    if !fingerprint.contains(':') || kind.is_empty() {
        return None;
    }
    Some((tokens[0].parse().ok(), fingerprint.to_owned(), kind.to_owned()))
}

/// The kind ssh-keygen would name for a public key type, for when it can't be asked.
pub fn kind_from_type(t: &str) -> String {
    match t {
        "ssh-ed25519" => "ED25519".into(),
        "ssh-rsa" => "RSA".into(),
        "ssh-dss" => "DSA".into(),
        "sk-ssh-ed25519@openssh.com" => "ED25519-SK".into(),
        _ if t.starts_with("sk-ecdsa-") => "ECDSA-SK".into(),
        _ if t.starts_with("ecdsa-sha2-") => "ECDSA".into(),
        _ => t.to_uppercase(),
    }
}

/// `ssh-keygen -l -f <public>`: the size, fingerprint and kind, or None if it fails.
pub fn fingerprint(keygen: &Path, public: &Path) -> Option<(Option<u32>, String, String)> {
    let mut cmd = command(keygen);
    cmd.arg("-l").arg("-f").arg(public);
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    parse_fingerprint(&String::from_utf8_lossy(&out.stdout))
}

/// The pair whose private key is `private`, if it and a parseable `.pub` exist.
pub(super) fn read_pair(private: &Path, keygen: Option<&Path>) -> Option<SshKeyPair> {
    if !private.is_file() {
        return None;
    }
    let public = pub_path(private);
    let text = std::fs::read_to_string(&public).ok()?;
    pair_from(private, public, &text, keygen)
}

/// The pair built from the `.pub` file's `text`, with its fingerprint when `keygen` is given;
/// None when the text isn't a public key line.
fn pair_from(private: &Path, public: PathBuf, text: &str, keygen: Option<&Path>) -> Option<SshKeyPair> {
    let (key_type, comment) = parse_public_key(text)?;
    let mut pair = SshKeyPair {
        private: private.to_owned(),
        kind: kind_from_type(&key_type),
        key_type,
        bits: None,
        fingerprint: None,
        comment,
        public_key: text.lines().next().unwrap_or_default().trim().to_owned(),
        public,
    };
    if let Some((bits, fp, kind)) = keygen.and_then(|k| fingerprint(k, &pair.public)) {
        pair.bits = bits;
        pair.fingerprint = Some(fp);
        pair.kind = kind;
    }
    Some(pair)
}

/// The file names ssh tries when no IdentityFile is given (ssh_config(5)), in its .ssh folder.
/// DSA keys are left out: OpenSSH turned them off by default and later removed them.
pub const DEFAULT_IDENTITY_NAMES: [&str; 5] = ["id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_ed25519", "id_ed25519_sk"];

/// Whether ssh tries `pair` on its own: it is in `dir` (the .ssh folder) under one of the
/// default names, spelled exactly (never case-folded).
pub fn is_default_identity(pair: &SshKeyPair, dir: &Path) -> bool {
    pair.private.parent().is_some_and(|p| same_spelling(p, dir))
        && pair.private.file_name().and_then(|n| n.to_str()).is_some_and(|n| DEFAULT_IDENTITY_NAMES.contains(&n))
}

/// Why a picked file isn't a usable key pair (Use existing).
#[derive(Clone, Debug, PartialEq)]
pub enum KeyFileProblem {
    /// The private key's path isn't a file (a folder, or gone).
    NotAFile,
    /// No `<private>.pub` next to it.
    NoPublicKey,
    /// The `.pub` file doesn't start with a public key line.
    NotAPublicKey,
    /// The `.pub` file couldn't be read.
    Unreadable(String),
}

/// The pair for a file the user picked, anywhere: a `.pub` file whose private key is next to it
/// stands for that key. Read exactly as the listing reads a pair.
pub fn read_key_pair(picked: &Path, keygen: Option<&Path>) -> Result<SshKeyPair, KeyFileProblem> {
    let stem = picked.extension().is_some_and(|x| x == "pub").then(|| picked.with_extension(""));
    let private = stem.filter(|s| s.is_file()).unwrap_or_else(|| picked.to_owned());
    if !private.is_file() {
        return Err(KeyFileProblem::NotAFile);
    }
    let public = pub_path(&private);
    let text = match std::fs::read_to_string(&public) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(KeyFileProblem::NoPublicKey),
        Err(e) => return Err(KeyFileProblem::Unreadable(e.to_string())),
    };
    pair_from(&private, public, &text, keygen).ok_or(KeyFileProblem::NotAPublicKey)
}

/// The key pairs in `dir`, sorted by file name; fingerprints only when `keygen` is given. A
/// missing folder has none.
pub fn list_key_pairs(dir: &Path, keygen: Option<&Path>) -> Result<Vec<SshKeyPair>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut keys: Vec<SshKeyPair> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pub"))
        .filter_map(|public| read_pair(&public.with_extension(""), keygen))
        .collect();
    keys.sort_by_key(SshKeyPair::file_name);
    Ok(keys)
}
