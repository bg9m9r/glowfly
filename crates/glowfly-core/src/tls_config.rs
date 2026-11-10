//! The TLS settings git uses for an HTTPS URL, read for a certificate or revocation failure's
//! Details, with where each is set: `http.sslBackend`, `http.sslCAInfo` (or the
//! `GIT_SSL_CAINFO` that overrides it), `http.schannelUseSSLCAInfo` and
//! `http.schannelCheckRevoke`. Under schannel git hands curl no CA file unless
//! `http.schannelUseSSLCAInfo` is true, so a set `http.sslCAInfo` can be ignored; `best-effort`
//! revocation checks need git 2.43 (an older git dies on the non-boolean value). Parsing and
//! resolving are pure; [`read`] runs `git config` (runner threads only).

use crate::external_tools::git_config::parse_z;
use crate::git_version::Version;
use crate::gitcli;
use std::path::Path;

/// Which kind of TLS failure a job had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlsFailure {
    /// git couldn't verify the server's certificate.
    Certificate,
    /// Schannel couldn't finish the revocation check.
    Revocation,
}

/// The config scope a value comes from, as `--show-scope` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    System,
    Global,
    Local,
    Worktree,
    Command,
}

/// Where a value is set, as `--show-origin` (and `--show-scope`) say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A config file: `path` exactly as git prints it (the repository's is `.git/config`);
    /// `scope` is None for a git too old to say (before 2.26).
    File { scope: Option<Scope>, path: String },
    /// A `-c` option or `GIT_CONFIG_PARAMETERS`.
    CommandLine,
    /// Anything else git names (a blob, standard input), or empty when no entry matched.
    Other(String),
}

/// One setting in use: its value, where it is set, and the URL of the `http.<url>.<var>` key
/// it came from (None for a plain `http.<var>`). A valueless key's value is "true".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub value: String,
    pub origin: Origin,
    pub url: Option<String>,
}

/// The TLS settings git uses; None where a key isn't set.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TlsSettings {
    pub backend: Option<Setting>,
    pub ca_info: Option<Setting>,
    pub use_ca_info: Option<Setting>,
    pub check_revoke: Option<Setting>,
    /// `GIT_SSL_CAINFO` in Glowfly's environment, which overrides `http.sslCAInfo`.
    pub ca_info_env: Option<String>,
}

/// The variables read, lowercase as git lists them.
const VARS: [&str; 4] = ["sslbackend", "sslcainfo", "schannelusesslcainfo", "schannelcheckrevoke"];

/// The keys read, as `git config --get-regexp` matches them: `http.<var>` or `http.<url>.<var>`.
pub const KEYS_RE: &str = r"^http\.(.+\.)?(sslbackend|sslcainfo|schannelusesslcainfo|schannelcheckrevoke)$";

/// The oldest git (major, minor) that takes `best-effort` for `http.schannelCheckRevoke`.
pub const BEST_EFFORT_SINCE: (u32, u32) = (2, 43);

/// Whether git `v` takes `best-effort`; an unknown version counts as new enough.
pub fn best_effort_supported(v: Option<Version>) -> bool {
    v.is_none_or(|v| (v.0, v.1) >= BEST_EFFORT_SINCE)
}

/// How the views spell a variable: "http.sslBackend" for "sslbackend".
pub fn shown_key(var: &str) -> &'static str {
    match var {
        "sslbackend" => "http.sslBackend",
        "sslcainfo" => "http.sslCAInfo",
        "schannelusesslcainfo" => "http.schannelUseSSLCAInfo",
        _ => "http.schannelCheckRevoke",
    }
}

/// One entry of `--show-origin --get-regexp` output; `value` None for a valueless key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub scope: Option<Scope>,
    pub origin: Origin,
    pub key: String,
    pub value: Option<String>,
}

/// Reads `git config -z --show-origin [--show-scope] --get-regexp` output: per entry the scope
/// (when `scoped`), the origin and `key\nvalue` (or a bare `key`), each ending in NUL.
pub fn parse_listing(out: &str, scoped: bool) -> Vec<Listed> {
    let fields: Vec<&str> = out.split('\0').collect();
    let width = if scoped { 3 } else { 2 };
    fields
        .chunks_exact(width)
        .map(|c| {
            let scope = if scoped { scope_of(c[0]) } else { None };
            let (origin, entry) = (c[width - 2], c[width - 1]);
            let origin = if (scoped && c[0] == "command") || origin == "command line:" {
                Origin::CommandLine
            } else if let Some(path) = origin.strip_prefix("file:") {
                Origin::File { scope, path: path.to_owned() }
            } else {
                Origin::Other(origin.to_owned())
            };
            let (key, value) = match entry.split_once('\n') {
                Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
                None => (entry.to_owned(), None),
            };
            Listed { scope, origin, key, value }
        })
        .collect()
}

fn scope_of(s: &str) -> Option<Scope> {
    match s {
        "system" => Some(Scope::System),
        "global" => Some(Scope::Global),
        "local" => Some(Scope::Local),
        "worktree" => Some(Scope::Worktree),
        "command" => Some(Scope::Command),
        _ => None,
    }
}

/// Reads `git config -z --get-urlmatch http <url>` output: the values in effect for the URL.
pub fn parse_effective(out: &str) -> Vec<(String, Option<String>)> {
    parse_z(out)
}

/// The settings in use: each value from `effective` (`--get-urlmatch`) when there is one, else
/// the last plain `http.<var>` listed; its origin from the last listed entry for that variable
/// with the same value.
pub fn resolve(listed: &[Listed], effective: Option<&[(String, Option<String>)]>) -> TlsSettings {
    let value_of = |v: &Option<String>| v.clone().unwrap_or_else(|| "true".to_owned());
    let setting = |var: &str| -> Option<Setting> {
        let plain = format!("http.{var}");
        let value = match effective {
            Some(e) => e.iter().rev().find(|(k, _)| *k == plain).map(|(_, v)| value_of(v)),
            None => listed.iter().rev().find(|l| l.key == plain).map(|l| value_of(&l.value)),
        }?;
        // The URL of an `http.<url>.<var>` key; without `effective` only plain keys apply.
        let sub = |key: &str| -> Option<Option<String>> {
            let rest = key.strip_prefix("http.")?;
            let (head, last) = rest.rsplit_once('.').unwrap_or(("", rest));
            let url = (!head.is_empty()).then(|| head.to_owned());
            (last.eq_ignore_ascii_case(var) && (url.is_none() || effective.is_some())).then_some(url)
        };
        let from = listed.iter().rev().find_map(|l| sub(&l.key).filter(|_| value_of(&l.value) == value).map(|url| (l, url)));
        Some(match from {
            Some((l, url)) => Setting { value, origin: l.origin.clone(), url },
            None => Setting { value, origin: Origin::Other(String::new()), url: None },
        })
    };
    let [backend, ca_info, use_ca_info, check_revoke] = VARS.map(setting);
    TlsSettings { backend, ca_info, use_ca_info, check_revoke, ca_info_env: None }
}

impl TlsSettings {
    /// Whether `http.sslBackend` is exactly "schannel", as git compares it.
    pub fn schannel(&self) -> bool {
        self.backend.as_ref().is_some_and(|b| b.value == "schannel")
    }

    /// Whether `http.schannelUseSSLCAInfo` is true (a value git can't read as a boolean isn't).
    pub fn uses_ca_info(&self) -> bool {
        self.use_ca_info.as_ref().is_some_and(|s| git2::Config::parse_bool(&s.value).unwrap_or(false))
    }

    /// Whether git leaves the CA file out: schannel chosen by `http.sslBackend`, a CA file set
    /// (in config or `GIT_SSL_CAINFO`) and `http.schannelUseSSLCAInfo` not true.
    pub fn ca_info_ignored(&self) -> bool {
        self.schannel() && (self.ca_info.is_some() || self.ca_info_env.is_some()) && !self.uses_ca_info()
    }
}

/// Reads the TLS settings git uses in `cwd`, for `url` when given (an http(s) URL without
/// credentials). `env` is added to git's environment. `ca_info_env` is left None. The error is
/// git's first line. Runner threads only.
pub fn read(cwd: &Path, url: Option<&str>, env: &[(&str, &str)]) -> Result<TlsSettings, String> {
    let vars: Vec<(&str, &str)> = gitcli::NO_PROMPTS.iter().chain(env).copied().collect();
    let run = |a: &[&str]| gitcli::output(cwd, &gitcli::args(a), "", &vars, Some(gitcli::READ_TIMEOUT));
    let first_line =
        |out: &gitcli::GitOutput| out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git config failed").to_owned();
    let mut scoped = true;
    let mut out = run(&["config", "-z", "--show-origin", "--show-scope", "--get-regexp", KEYS_RE]);
    if out.code == Some(129) {
        // --show-scope came in git 2.26.
        scoped = false;
        out = run(&["config", "-z", "--show-origin", "--get-regexp", KEYS_RE]);
    }
    let listed = match out.code {
        Some(0) => parse_listing(&out.stdout, scoped),
        Some(1) => Vec::new(),
        _ => return Err(first_line(&out)),
    };
    let effective = match url {
        Some(u) => {
            let out = run(&["config", "-z", "--get-urlmatch", "http", u]);
            match out.code {
                Some(0) => Some(parse_effective(&out.stdout)),
                Some(1) => Some(Vec::new()),
                _ => return Err(first_line(&out)),
            }
        }
        None => None,
    };
    Ok(resolve(&listed, effective.as_deref()))
}

#[cfg(test)]
#[path = "tls_config_tests.rs"]
mod tests;
