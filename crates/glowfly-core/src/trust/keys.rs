//! Which config settings are a reason to ask before git runs in a repository (`command_key`):
//! those that make git or git-lfs run a program, and those that change how git connects to a
//! server (TLS checks and proxies, `network_key`), which a repository from elsewhere shouldn't
//! steer unasked since its sign-ins travel over them.

use git2::Config;
use std::borrow::Cow;

/// Single keys that name a program, as libgit2 returns them (lowercase), with their usual spelling.
const COMMAND_KEYS: &[(&str, &str)] = &[
    ("core.sshcommand", "core.sshCommand"),
    ("core.hookspath", "core.hooksPath"),
    ("core.editor", "core.editor"),
    ("core.askpass", "core.askPass"),
    ("core.gitproxy", "core.gitProxy"),
    ("sequence.editor", "sequence.editor"),
    ("diff.external", "diff.external"),
    ("credential.helper", "credential.helper"),
    ("gpg.program", "gpg.program"),
    ("gpg.openpgp.program", "gpg.openpgp.program"),
    ("gpg.x509.program", "gpg.x509.program"),
    ("gpg.ssh.program", "gpg.ssh.program"),
    ("gpg.ssh.defaultkeycommand", "gpg.ssh.defaultKeyCommand"),
    // Run to list an alternate's refs during fetch negotiation.
    ("core.alternaterefscommand", "core.alternateRefsCommand"),
    // Picks which `lfs.customtransfer.<name>` program git-lfs runs for every transfer.
    ("lfs.standalonetransferagent", "lfs.standaloneTransferAgent"),
];

/// The `http.*` and `http.<url>.*` keys that weaken TLS checks or send traffic through a proxy,
/// as libgit2 returns them, with their usual spelling.
const HTTP_KEYS: &[(&str, &str)] =
    &[("sslverify", "sslVerify"), ("sslcainfo", "sslCAInfo"), ("sslcapath", "sslCAPath"), ("proxy", "proxy")];

/// The git commands Glowfly runs that aren't built into git, so a repository's alias of the
/// same name runs in their place when the program is missing (git looks for `git-<name>`
/// before aliases, but after its built-in commands, which an alias never replaces).
const ALIASED_COMMANDS: &[&str] = &["lfs", "mergetool"];

/// The display key when the setting `name` = `value` makes git run a program, names a remote
/// git could read as an option (the snapshot leaves that remote out), or changes how it
/// connects (`network_key`, whatever its value).
pub(super) fn command_key(name: &str, value: &str) -> Option<String> {
    if let Some(key) = network_key(name) {
        return Some(key);
    }
    // An empty value resets a list (credential.helper) or turns the setting off.
    if value.is_empty() {
        return None;
    }
    if let Some((_, shown)) = COMMAND_KEYS.iter().find(|(k, _)| *k == name) {
        return Some((*shown).to_owned());
    }
    // git runs an alias for a command it can't find (alias.lfs when git-lfs isn't installed),
    // in any letter case of the name.
    if let Some(cmd) = name.strip_prefix("alias.").map(|rest| rest.strip_suffix(".command").unwrap_or(rest)) {
        let shadows = ALIASED_COMMANDS.iter().any(|c| c.eq_ignore_ascii_case(cmd));
        return shadows.then(|| format!("alias.{}", cmd.to_ascii_lowercase()));
    }
    if name == "protocol.allow" || name == "remote.pushdefault" {
        let on = if name == "protocol.allow" { value.eq_ignore_ascii_case("always") } else { value.starts_with('-') };
        return on.then(|| name.to_owned());
    }
    if name == "core.fsmonitor" {
        // A boolean means git's built-in daemon; anything else is a hook's path.
        return Config::parse_bool(value).is_err().then(|| name.to_owned());
    }
    let (first, last) = (name.find('.')?, name.rfind('.')?);
    if first == last {
        return None;
    }
    let (section, sub, key) = (&name[..first], &name[first + 1..last], &name[last + 1..]);
    let runs = match section {
        "credential" => key == "helper",
        "diff" => matches!(key, "command" | "textconv"),
        "merge" => key == "driver",
        // `git submodule update` runs `!command` (only from the repository's own config).
        "submodule" => key == "update" && value.starts_with('!'),
        "filter" => matches!(key, "clean" | "smudge" | "process"),
        // git difftool / mergetool start a tool's own command, or a built-in tool from `.path`.
        "difftool" | "mergetool" => matches!(key, "cmd" | "path"),
        "remote" if sub.starts_with('-') => true,
        "remote" => matches!(key, "uploadpack" | "receivepack") || (matches!(key, "url" | "pushurl") && runs_transport(value)),
        // A branch's remote can be a URL.
        "branch" => matches!(key, "remote" | "pushremote") && (runs_transport(value) || value.starts_with('-')),
        // Rewrites matching URLs to the subsection.
        "url" => matches!(key, "insteadof" | "pushinsteadof") && runs_transport(sub),
        // Lets ext:: run, including for submodule URLs from .gitmodules.
        "protocol" => sub == "ext" && key == "allow" && value != "never",
        // libgit2 reads a conditional include only for the conditions it knows; git may read
        // others (hasconfig:), so what such a file sets is unseen.
        "includeif" => key == "path" && !["gitdir:", "gitdir/i:", "onbranch:"].iter().any(|c| sub.starts_with(c)),
        // git-lfs runs a custom transfer agent (`path`, with its `args`) and an extension's
        // clean and smudge commands. The subsection is as written, so any letter case.
        "lfs" => match sub.split_once('.').map(|(kind, _)| kind.to_ascii_lowercase()).as_deref() {
            Some("customtransfer") => matches!(key, "path" | "args"),
            Some("extension") => matches!(key, "clean" | "smudge"),
            _ => false,
        },
        _ => false,
    };
    if !runs {
        return None;
    }
    Some(match section {
        "includeif" => format!("includeIf.{sub}.path"),
        "url" => format!("url.{sub}.{}", if key == "insteadof" { "insteadOf" } else { "pushInsteadOf" }),
        _ => name.to_owned(),
    })
}

/// The display key when `name` (as libgit2 returns it: section and key lowercase) changes how
/// git connects: TLS checks and proxies, for every URL (`http.sslVerify`) or one
/// (`http.<url>.sslVerify`), and a remote's proxy. Any value counts, `true` and a bare host
/// too: the repository shouldn't pick where sign-ins go or which certificates vouch for them.
pub(super) fn network_key(name: &str) -> Option<String> {
    let (section, rest) = name.split_once('.')?;
    let (sub, key) = match rest.rfind('.') {
        Some(i) => (Some(&rest[..i]), &rest[i + 1..]),
        None => (None, rest),
    };
    match section {
        "http" => {
            let (_, shown) = HTTP_KEYS.iter().find(|(k, _)| *k == key)?;
            Some(sub.map_or_else(|| format!("http.{shown}"), |s| format!("http.{s}.{shown}")))
        }
        "remote" => (sub.is_some() && key == "proxy").then(|| name.to_owned()),
        _ => None,
    }
}

/// Whether the listed setting `key` (`ConfigCommand::key`) is one that changes how git
/// connects rather than one that runs a program: the trust prompt lists those apart.
pub fn is_network_setting(key: &str) -> bool {
    let lower = match key.split_once('.') {
        // The section and the last part, lowercase, as libgit2 gives them; a subsection as written.
        Some((section, rest)) => match rest.rfind('.') {
            Some(i) => format!("{}.{}.{}", section.to_ascii_lowercase(), &rest[..i], rest[i + 1..].to_ascii_lowercase()),
            None => format!("{}.{}", section.to_ascii_lowercase(), rest.to_ascii_lowercase()),
        },
        None => return false,
    };
    network_key(&lower).is_some()
}

/// Config bytes as text: as written when they are UTF-8; otherwise each byte that isn't is
/// U+FFFD and its two hex digits, and a U+FFFD written in the text is doubled, so two different
/// byte strings never read the same (the print compares the text).
pub(super) fn text_of(bytes: &[u8]) -> Cow<'_, str> {
    const MARK: char = '\u{FFFD}';
    let doubled = |s: &str| s.replace(MARK, "\u{FFFD}\u{FFFD}");
    match std::str::from_utf8(bytes) {
        Ok(s) if !s.contains(MARK) => Cow::Borrowed(s),
        Ok(s) => Cow::Owned(doubled(s)),
        Err(_) => {
            let mut out = String::new();
            for chunk in bytes.utf8_chunks() {
                out.push_str(&doubled(chunk.valid()));
                for b in chunk.invalid() {
                    out.push_str(&format!("{MARK}{b:02X}"));
                }
            }
            Cow::Owned(out)
        }
    }
}

/// Whether git reaching `url` runs a command: the ext:: transport (any case, since a Windows
/// file system finds its helper under any spelling).
pub(super) fn runs_transport(url: &str) -> bool {
    url.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("ext::"))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
