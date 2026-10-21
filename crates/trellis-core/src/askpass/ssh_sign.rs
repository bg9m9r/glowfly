//! ssh-keygen's passphrase prompt while git signs with an SSH key. Pure: no I/O.
//!
//! With `gpg.format=ssh` git runs `ssh-keygen -Y sign -f <user.signingKey> …` itself. When the
//! key isn't in ssh-agent and its private half has a passphrase, ssh-keygen asks through
//! `SSH_ASKPASS` like ssh does, but in its own wording. gitcli marks the commands that may sign
//! with the configured key file ([`super::protocol::ENV_SSH_SIGNING_KEY`]); the helper uses that
//! marker and the wording to send an [`SshSignAsk`] instead of a plain prompt.

use super::protocol::SshSignAsk;

/// The signing request for ssh-keygen's `prompt`, when gitcli set the signing-key `marker`
/// (the `user.signingKey` file). ssh-keygen says `Enter passphrase:` (older versions) or
/// `Enter passphrase for "<private key>":`; ssh's `for key '…'` and ssh-add's unquoted form are
/// not signing prompts and give None.
pub fn ask_for(prompt: &str, marker: Option<&str>) -> Option<SshSignAsk> {
    let marker = marker.filter(|m| !m.is_empty())?;
    let t = prompt.trim_end();
    let t = t.strip_suffix(':').unwrap_or(t).trim_end();
    if t == "Enter passphrase" {
        return Some(SshSignAsk { key: private_key_path(marker) });
    }
    let quoted = t.strip_prefix("Enter passphrase for \"")?.strip_suffix('"')?;
    let key = if quoted.is_empty() { private_key_path(marker) } else { quoted.to_owned() };
    Some(SshSignAsk { key })
}

/// The private key ssh-keygen loads for the key file `path`: a trailing `-cert.pub` or `.pub`
/// removed.
pub fn private_key_path(path: &str) -> String {
    path.strip_suffix("-cert.pub").or_else(|| path.strip_suffix(".pub")).unwrap_or(path).to_owned()
}

/// Whether a failed command's `output` says ssh-keygen couldn't unlock the signing key with the
/// passphrase given.
pub fn wrong_passphrase(output: &str) -> bool {
    output.contains("incorrect passphrase supplied to decrypt private key")
}
