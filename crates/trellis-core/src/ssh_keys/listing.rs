//! Reading key pairs: public key lines, `ssh-keygen -l` fingerprints, and the pairs in a folder.

use super::{SshKeyPair, pub_path};
use crate::error::Result;
use crate::signing::command;
use std::path::Path;

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
    let (key_type, comment) = parse_public_key(&text)?;
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
