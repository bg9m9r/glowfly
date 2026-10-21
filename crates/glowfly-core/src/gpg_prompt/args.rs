//! Reading git's arguments for gpg and building the wrapper's own. Pure.

use std::ffi::OsString;

/// A call that asks gpg to sign: the key git named (`-u`), and git's arguments as given.
#[derive(Clone, Debug, PartialEq)]
pub struct SignCall {
    pub key: Option<String>,
    pub original: Vec<OsString>,
}

/// Some for git's sign call (`--status-fd=2` with `-bsau <key>`, `--sign` or `--detach-sign`);
/// None for a verification or anything else.
pub fn parse(args: &[OsString]) -> Option<SignCall> {
    let text: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let (mut status, mut sign, mut key) = (false, false, None);
    let mut i = 0;
    while i < text.len() {
        let a = text[i].as_str();
        match a {
            "--" => break,
            "--status-fd=2" => status = true,
            "--sign" | "--detach-sign" => sign = true,
            "--verify" => return None,
            "--local-user" => {
                key = text.get(i + 1).cloned();
                i += 1;
            }
            _ if a.starts_with("--local-user=") => key = Some(a["--local-user=".len()..].to_owned()),
            _ if a.len() > 1 && a.starts_with('-') && !a.starts_with("--") => {
                let cluster = &a[1..];
                sign |= cluster.contains('s');
                if cluster.ends_with('u') {
                    key = text.get(i + 1).cloned();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    (status && sign).then(|| SignCall { key, original: args.to_vec() })
}

fn with(prefix: &[&str], call: &SignCall) -> Vec<OsString> {
    prefix.iter().map(OsString::from).chain(call.original.iter().cloned()).collect()
}

/// The first try: fail rather than start a pinentry, so a cached key signs and an uncached
/// one reports "No pinentry".
pub fn probe(call: &SignCall) -> Vec<OsString> {
    with(&["--pinentry-mode", "error"], call)
}

/// Signing with the passphrase read from stdin's first line, the payload after it.
pub fn loopback(call: &SignCall) -> Vec<OsString> {
    with(&["--batch", "--pinentry-mode", "loopback", "--passphrase-fd", "0"], call)
}

/// Listing the secret key git named, for the dialog.
pub fn list_key(key: &str) -> Vec<OsString> {
    ["--batch", "--list-secret-keys", "--with-colons", "--keyid-format=long", "--", key].iter().map(OsString::from).collect()
}
