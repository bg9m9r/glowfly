//! A remote's section in the repository's own config file and the branch settings that name it:
//! read before a remote is removed or renamed, and written back value by value with
//! `git config --add`, so repeated keys (several URLs or refspecs) keep their order. Only the
//! repository's config file counts (`ConfigLevel::Local`, not what it includes): that is the
//! file `git remote remove` edits.

use super::record::local_names;
use crate::error::Result;
use crate::gitcli::args;
use git2::{ConfigLevel, Repository};

/// Everything a remote's removal drops from the repository's config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteConfig {
    /// Every `remote.<name>.<key>` value in the repository's config file, in file order: the
    /// key lowercased as libgit2 reports it, the value as written ("true" for a value-less key).
    pub entries: Vec<(String, String)>,
    /// Local branches whose settings named it: `branch.<b>.remote` (with `.merge`), `.pushremote`.
    pub branches: Vec<BranchLink>,
    /// `remote.pushDefault` named it.
    pub push_default: bool,
}

/// One branch setting that named a remote: `branch.<branch>.<key>` set to `value`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchLink {
    pub branch: String,
    /// "remote", "merge" or "pushremote".
    pub key: String,
    pub value: String,
}

/// A remote's URLs before and after an edit (only `url` and `pushurl` entries, in order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlChange {
    pub before: Vec<(String, String)>,
    pub after: Vec<(String, String)>,
}

/// The branch keys read for the links, lowercased as libgit2 reports them.
const LINK_KEYS: [&str; 3] = ["remote", "merge", "pushremote"];

/// Remote `name`'s section and the branch settings naming it, from the repository's own config
/// file. None when the file has no `remote.<name>.*` value, or when a value that would have to be
/// written back isn't UTF-8 (it couldn't be passed back exactly).
pub(crate) fn read(repo: &Repository, name: &str) -> Result<Option<RemoteConfig>> {
    let cfg = repo.config()?.open_level(ConfigLevel::Local)?.snapshot()?;
    let prefix = format!("remote.{name}.");
    let mut entries = Vec::new();
    // The last value of each branch key (as git reads a single-valued key); None: not UTF-8.
    let mut links: Vec<(String, String, Option<String>)> = Vec::new();
    let mut push_default = None;
    let mut it = cfg.entries(None)?;
    while let Some(e) = it.next() {
        let e = e?;
        if e.include_depth() > 0 {
            continue;
        }
        let Ok(full) = std::str::from_utf8(e.name_bytes()) else {
            if e.name_bytes().starts_with(prefix.as_bytes()) {
                return Ok(None);
            }
            continue;
        };
        let value = || crate::config_entry::raw(e).map_or(Some("true"), |b| std::str::from_utf8(b).ok()).map(str::to_owned);
        // The subsection is case-sensitive and may hold dots, so the prefix is matched exactly.
        if let Some(key) = full.strip_prefix(&prefix).filter(|k| !k.is_empty() && !k.contains('.')) {
            let Some(v) = value() else { return Ok(None) };
            entries.push((key.to_owned(), v));
        } else if full == "remote.pushdefault" {
            push_default = Some(value());
        } else if let Some(rest) = full.strip_prefix("branch.")
            && let Some((branch, key)) = rest.rsplit_once('.')
            && !branch.is_empty()
            && LINK_KEYS.contains(&key)
        {
            links.retain(|(b, k, _)| !(b == branch && k == key));
            links.push((branch.to_owned(), key.to_owned(), value()));
        }
    }
    if entries.is_empty() {
        return Ok(None);
    }
    let names = |key: &str, b: &str| links.iter().any(|(lb, k, v)| lb == b && k == key && v.as_deref() == Some(name));
    let mut branches = Vec::new();
    for (branch, key, value) in &links {
        let keep = match key.as_str() {
            "remote" | "pushremote" => value.as_deref() == Some(name),
            _ => names("remote", branch),
        };
        if !keep {
            continue;
        }
        let Some(value) = value else { return Ok(None) };
        branches.push(BranchLink { branch: branch.clone(), key: key.clone(), value: value.clone() });
    }
    Ok(Some(RemoteConfig { entries, branches, push_default: push_default.flatten().as_deref() == Some(name) }))
}

/// The `url` and `pushurl` entries of `c`, in order.
pub(crate) fn urls(c: &RemoteConfig) -> Vec<(String, String)> {
    c.entries.iter().filter(|(k, _)| k == "url" || k == "pushurl").cloned().collect()
}

/// The commands that write remote `name`'s section `c` back, then each branch link whose branch
/// still exists and whose key is unset now, then `remote.pushDefault` when it is unset now.
pub(super) fn restore_cmds(name: &str, c: &RemoteConfig, repo: &Repository) -> Result<Vec<Vec<String>>> {
    let mut cmds: Vec<Vec<String>> = c.entries.iter().map(|(k, v)| args(&["config", "--add", &format!("remote.{name}.{k}"), v])).collect();
    let cfg = repo.config()?.open_level(ConfigLevel::Local)?.snapshot()?;
    let unset = |key: &str| cfg.get_entry(key).is_err();
    let branches = local_names(repo)?;
    for l in &c.branches {
        let key = format!("branch.{}.{}", l.branch, l.key);
        if branches.contains(&l.branch) && unset(&key) {
            cmds.push(args(&["config", &key, &l.value]));
        }
    }
    if c.push_default && unset("remote.pushdefault") {
        cmds.push(args(&["config", "remote.pushDefault", name]));
    }
    Ok(cmds)
}

/// The commands that turn remote `name`'s `url` and `pushurl` values from `from` into `to`: for
/// each key whose values differ, all of them unset (when `from` has any), then `to`'s added in
/// order.
pub(super) fn set_urls_cmds(name: &str, from: &[(String, String)], to: &[(String, String)]) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    for key in ["url", "pushurl"] {
        let values = |list: &[(String, String)]| list.iter().filter(|(k, _)| k == key).map(|(_, v)| v.clone()).collect::<Vec<_>>();
        let (old, new) = (values(from), values(to));
        if old == new {
            continue;
        }
        let full = format!("remote.{name}.{key}");
        if !old.is_empty() {
            cmds.push(args(&["config", "--unset-all", &full]));
        }
        cmds.extend(new.iter().map(|v| args(&["config", "--add", &full, v])));
    }
    cmds
}
