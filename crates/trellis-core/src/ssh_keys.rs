//! SSH key pairs in `~/.ssh`: listing them with their `ssh-keygen` fingerprints, and generating
//! Ed25519 keys with `ssh-keygen`. Trellis keeps no key material; it reads the files ssh uses
//! and runs ssh's own tool to make new ones.

use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use crate::signing::command;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// A private key in the .ssh folder with a matching, readable `.pub` file next to it.
#[derive(Clone, Debug, PartialEq)]
pub struct SshKeyPair {
    pub private: PathBuf,
    pub public: PathBuf,
    /// The first token of the `.pub` line, e.g. "ssh-ed25519".
    pub key_type: String,
    /// "ED25519", "RSA", "ECDSA", "ED25519-SK", …
    pub kind: String,
    pub bits: Option<u32>,
    /// "SHA256:…", from `ssh-keygen -l`; None when it couldn't be read.
    pub fingerprint: Option<String>,
    pub comment: String,
    /// The `.pub` line, trimmed: what Copy public key puts on the clipboard.
    pub public_key: String,
}

impl SshKeyPair {
    pub fn file_name(&self) -> String {
        self.private.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// The kind, with the size for key types that come in several sizes ("RSA 3072").
    pub fn kind_label(&self) -> String {
        match self.bits {
            Some(bits) if matches!(self.kind.as_str(), "RSA" | "ECDSA" | "DSA") => format!("{} {bits}", self.kind),
            _ => self.kind.clone(),
        }
    }
}

/// What Preferences > SSH shows: the key pairs and whether ssh-keygen was found.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SshListing {
    pub home: PathBuf,
    /// home/.ssh
    pub dir: PathBuf,
    pub keys: Vec<SshKeyPair>,
    pub keygen: Option<PathBuf>,
}

/// A key to generate. Its `Debug` output never shows the passphrase.
#[derive(Clone, PartialEq)]
pub struct NewKey {
    /// The private key's path; the public key goes next to it with `.pub` added.
    pub path: PathBuf,
    /// Empty: ssh-keygen's default (user@host).
    pub comment: String,
    /// Empty: no passphrase.
    pub passphrase: String,
}

impl std::fmt::Debug for NewKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let passphrase = if self.passphrase.is_empty() { "" } else { "<redacted>" };
        f.debug_struct("NewKey").field("path", &self.path).field("comment", &self.comment).field("passphrase", &passphrase).finish()
    }
}

/// Why a typed save location can't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationProblem {
    Empty,
    Relative,
    PublicFile,
    NotAFile,
}

impl LocationProblem {
    pub fn message(self) -> &'static str {
        match self {
            LocationProblem::Empty => "Enter where to save the private key.",
            LocationProblem::Relative => "Enter a full path, such as ~/.ssh/id_ed25519_work.",
            LocationProblem::PublicFile => "Enter the private key's path; the public key is saved next to it with .pub added.",
            LocationProblem::NotAFile => "Enter a file name, not a folder.",
        }
    }
}

pub fn ssh_dir(home: &Path) -> PathBuf {
    home.join(".ssh")
}

/// Where a new key goes unless the user says otherwise.
pub fn default_location(home: &Path) -> PathBuf {
    ssh_dir(home).join("id_ed25519")
}

/// The public key file of `private`: the same path with `.pub` added.
fn pub_path(private: &Path) -> PathBuf {
    let mut s = private.as_os_str().to_owned();
    s.push(".pub");
    PathBuf::from(s)
}

/// The key type and comment of a public key line (the first line of a `.pub` file). Lines
/// that don't start with a key type, such as authorized_keys entries with options, are None.
pub fn parse_public_key(text: &str) -> Option<(String, String)> {
    let mut tokens = text.lines().next()?.split_whitespace();
    let key_type = tokens.next()?;
    if !["ssh-", "ecdsa-sha2-", "sk-"].iter().any(|p| key_type.starts_with(p)) {
        return None;
    }
    tokens.next()?;
    Some((key_type.to_owned(), tokens.collect::<Vec<_>>().join(" ")))
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

/// The private key path a typed save location means. `~/` (or `~\`) is the home folder. Pure:
/// nothing is read from the disk.
pub fn resolve_location(text: &str, home: &Path) -> Result<PathBuf, LocationProblem> {
    let t = text.trim();
    if t.is_empty() {
        return Err(LocationProblem::Empty);
    }
    if t == "~" || t.ends_with('/') || t.ends_with('\\') {
        return Err(LocationProblem::NotAFile);
    }
    let path = match t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        Some(rest) => rest.split(['/', '\\']).filter(|s| !s.is_empty()).fold(home.to_path_buf(), |p, s| p.join(s)),
        None => PathBuf::from(t),
    };
    if t.to_ascii_lowercase().ends_with(".pub") {
        return Err(LocationProblem::PublicFile);
    }
    if !path.is_absolute() {
        return Err(LocationProblem::Relative);
    }
    Ok(path)
}

/// Why `c` can't be a key comment: it would break the one-line public key.
pub fn comment_problem(c: &str) -> Option<&'static str> {
    c.chars().any(char::is_control).then_some("The comment can't contain line breaks or control characters.")
}

/// ssh-keygen's arguments for writing `new` to `out`. The passphrase is always given (empty for
/// none) so ssh-keygen never prompts; an empty comment leaves ssh-keygen's default.
///
/// ssh-keygen takes a new passphrase only as an argument or from a terminal, so it is briefly
/// visible in the process list to the same user or an administrator. It never reaches a log.
pub fn keygen_args(new: &NewKey, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-q", "-t", "ed25519", "-N"].into_iter().map(OsString::from).collect();
    a.push(new.passphrase.clone().into());
    if !new.comment.is_empty() {
        a.push("-C".into());
        a.push(new.comment.clone().into());
    }
    a.push("-f".into());
    a.push(out.as_os_str().to_owned());
    a
}

/// Where to look for ssh-keygen, in order: each PATH entry, Git for Windows' bundled copy
/// (three folders above git's exec path, in usr/bin), then Windows' own OpenSSH.
pub fn keygen_candidates(
    path_var: Option<&OsStr>,
    exe_suffix: &str,
    git_exec_path: Option<&Path>,
    system_root: Option<&Path>,
) -> Vec<PathBuf> {
    let name = format!("ssh-keygen{exe_suffix}");
    let mut out: Vec<PathBuf> = path_var
        .map(|p| std::env::split_paths(p).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(&name)).collect())
        .unwrap_or_default();
    if let Some(git_root) = git_exec_path.and_then(|p| p.ancestors().nth(3)) {
        out.push(git_root.join("usr").join("bin").join("ssh-keygen.exe"));
    }
    if let Some(root) = system_root {
        out.push(root.join("System32").join("OpenSSH").join("ssh-keygen.exe"));
    }
    out
}

/// The ssh-keygen program to run, if one is installed.
pub fn find_keygen(cwd: &Path) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH");
    let (exec_path, system_root) = if cfg!(windows) {
        let out = gitcli::read(cwd, &args(&["--exec-path"]));
        let exec = (out.ok && !out.stdout.trim().is_empty()).then(|| PathBuf::from(out.stdout.trim()));
        (exec, std::env::var_os("SystemRoot").map(PathBuf::from))
    } else {
        (None, None)
    };
    keygen_candidates(path_var.as_deref(), std::env::consts::EXE_SUFFIX, exec_path.as_deref(), system_root.as_deref())
        .into_iter()
        .find(|p| p.is_file())
}

/// `ssh-keygen -l -f <public>`: the size, fingerprint and kind, or None if it fails.
pub fn fingerprint(keygen: &Path, public: &Path) -> Option<(Option<u32>, String, String)> {
    let mut cmd = command(keygen);
    cmd.arg("-l").arg("-f").arg(public);
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    parse_fingerprint(&String::from_utf8_lossy(&out.stdout))
}

/// The pair whose private key is `private`, if it and a parseable `.pub` exist.
fn read_pair(private: &Path, keygen: Option<&Path>) -> Option<SshKeyPair> {
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

/// Everything Preferences > SSH shows for the user whose home folder is `home`.
pub fn list(home: &Path, cwd: &Path) -> Result<SshListing> {
    let keygen = find_keygen(cwd);
    let dir = ssh_dir(home);
    let keys = list_key_pairs(&dir, keygen.as_deref())?;
    Ok(SshListing { home: home.to_owned(), dir, keys, keygen })
}

/// Creates `dir` and any missing parents; on Unix, private to the user (as ssh expects).
fn create_private_dir(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// Generates an Ed25519 key pair at `new.path` (and `.pub`). Without `overwrite`, an existing
/// file there is `Error::WouldOverwrite`. The pair is written under a temporary name first and
/// then moved into place, so a failure leaves any old pair as it was.
pub fn generate(keygen: &Path, new: &NewKey, overwrite: bool) -> Result<SshKeyPair> {
    let path = &new.path;
    if path.is_dir() {
        return Err(Error::Other(format!("{} is a folder", path.display())));
    }
    let public = pub_path(path);
    if !overwrite && (path.exists() || public.exists()) {
        return Err(Error::WouldOverwrite(path.display().to_string()));
    }
    let (Some(parent), Some(name)) = (path.parent().filter(|p| !p.as_os_str().is_empty()), path.file_name()) else {
        return Err(Error::Other(format!("{} is not a file path", path.display())));
    };
    create_private_dir(parent)?;
    let tmp = parent.join(format!(".{}.trellis-new-{}", name.to_string_lossy(), std::process::id()));
    let tmp_pub = pub_path(&tmp);
    let clean = || {
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&tmp_pub);
    };
    clean();
    let mut cmd = command(keygen);
    cmd.args(keygen_args(new, &tmp));
    let out = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            clean();
            return Err(Error::Other(format!("could not run ssh-keygen: {e}")));
        }
    };
    if !out.status.success() {
        clean();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let why = if stderr.is_empty() { out.status.to_string() } else { stderr };
        return Err(Error::Other(format!("ssh-keygen failed: {why}")));
    }
    // A rename replaces an existing file on Windows and Unix alike.
    if let Err(e) = std::fs::rename(&tmp, path).and_then(|()| std::fs::rename(&tmp_pub, &public)) {
        clean();
        return Err(e.into());
    }
    read_pair(path, Some(keygen)).ok_or_else(|| Error::Other("the new key was not found".into()))
}
