//! Profile folder rules in the global git config: for each rule an `[includeIf "gitdir:<folder>/"]`
//! entry whose `path` is the profile's include file, which Trellis writes in its own
//! `git-profiles` folder with the profile's `[user]` identity. Repositories under the folder
//! then commit as the profile, in Trellis and on the command line, while the global identity
//! and every repository's own config stay as they are.
//!
//! Entries are written shallow-first, so git's last include gives the deepest folder, as
//! `settings::profile_for_git_dir` expects. A sync rewrites the files that changed and, when
//! asked to place the entries, removes Trellis's own sections (those whose every key is a path
//! into the folder) and adds the plan's at the end of the global file, which is also what puts
//! them back after a later `[user]` section. Sections pointing anywhere else are never touched.
//! Writes go through the git CLI, one process at a time (they share the file's lock).

mod pattern;

pub use pattern::{FILE_HEADER, NOT_TEXT, config_path_value, file_text, gitdir_pattern};

use crate::gitcli::{self, args};
use crate::identity::Identity;
use crate::paths;
use crate::persist;
use crate::settings::{FolderRule, ProfileId, Settings};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The folder of the include files: `git-profiles` in Trellis's settings folder.
pub fn default_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("git-profiles"))
}

/// The include file's name for a profile.
pub fn file_name(id: ProfileId) -> String {
    format!("profile-{}.gitconfig", id.0)
}

/// The profile whose include file `origin` is (in `dir`, named by `file_name`).
pub fn profile_of_file(dir: &Path, origin: &Path) -> Option<ProfileId> {
    if !origin.parent().is_some_and(|p| paths::same_spelling(p, dir)) {
        return None;
    }
    let name = origin.file_name()?.to_str()?;
    let n = name.strip_prefix("profile-")?.strip_suffix(".gitconfig")?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse().ok().map(ProfileId)).flatten()
}

/// What a sync writes: an include file per profile with rules, and the entries in order.
#[derive(Clone, Debug, PartialEq)]
pub struct IncludePlan {
    pub files: Vec<(ProfileId, Identity)>,
    /// (`gitdir` pattern, profile), shallow-first.
    pub entries: Vec<(String, ProfileId)>,
}

/// The plan for `settings`' rules. A rule whose pattern can't be written is skipped (it was
/// checked when it was added).
pub fn plan(settings: &Settings) -> IncludePlan {
    let files = settings.profiles.iter().filter(|p| !p.folders.is_empty()).map(|p| (p.id, p.identity())).collect();
    let mut rules: Vec<(ProfileId, FolderRule)> = settings.rules();
    // Shallow first, then by text: git's last include wins, so the deepest folder applies.
    rules.sort_by_key(|(_, r)| (paths::decode(&r.path).components().count(), r.path.clone()));
    let entries = rules.iter().filter_map(|(id, r)| gitdir_pattern(r).ok().map(|p| (p, *id))).collect();
    IncludePlan { files, entries }
}

/// What a failed sync says, before git's own words.
const SYNC_FAILED: &str = "Could not update the profile folder rules in your Git config";

/// Writes the plan's include files into `dir` (only those whose text changed) and, with
/// `place`, puts the plan's entries at the end of the global config in place of Trellis's
/// earlier ones, then removes include files no profile has any more. Stops at the first
/// failure; the next sync does everything again.
pub fn sync(cwd: &Path, dir: &Path, plan: &IncludePlan, place: bool, env: &[(&str, &str)]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{SYNC_FAILED}: {e}");
    std::fs::create_dir_all(dir).map_err(io)?;
    for (id, identity) in &plan.files {
        let file = dir.join(file_name(*id));
        let text = file_text(identity);
        if std::fs::read(&file).ok().as_deref() != Some(text.as_bytes()) {
            persist::atomic_write(&file, text.as_bytes()).map_err(io)?;
        }
    }
    if !place {
        return Ok(());
    }
    let sections = sections(cwd, env)?;
    let mut kept: HashSet<(String, String)> = HashSet::new();
    for (sub, keys) in &sections {
        let ours = |(key, value): &(String, String)| key == "path" && paths::is_within(Path::new(value), dir);
        if !keys.iter().any(ours) {
            continue;
        }
        if keys.iter().all(ours) {
            git(cwd, &["config", "--global", "--remove-section", &format!("includeIf.{sub}")], env)?;
        } else {
            // Shared with the user's own keys: left as it is, and not added again.
            kept.extend(keys.iter().filter(|k| ours(k)).map(|(_, v)| (sub.clone(), v.clone())));
        }
    }
    for (pattern, id) in &plan.entries {
        let value = config_path_value(&dir.join(file_name(*id))).map_err(|e| format!("{SYNC_FAILED}: {e}"))?;
        if kept.iter().any(|(sub, v)| sub == pattern && paths::same_spelling(Path::new(v), Path::new(&value))) {
            continue;
        }
        git(cwd, &["config", "--global", "--add", &format!("includeIf.{pattern}.path"), &value], env)?;
    }
    let wanted: HashSet<String> = plan.files.iter().map(|(id, _)| file_name(*id)).collect();
    for entry in std::fs::read_dir(dir).map_err(io)?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if profile_of_file(dir, &dir.join(name)).is_some() && !wanted.contains(name) {
            persist::remove_file(&entry.path()).map_err(io)?;
        }
    }
    Ok(())
}

/// `includeIf` sections by subsection, each with its (key, value) pairs, in file order.
type Sections = Vec<(String, Vec<(String, String)>)>;

/// One git config write; its failure in the sync's words.
fn git(cwd: &Path, a: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let out = gitcli::output(cwd, &args(a), "", env, Some(gitcli::READ_TIMEOUT));
    if out.ok { Ok(()) } else { Err(format!("{SYNC_FAILED}: {}", out.combined())) }
}

/// The global file's `includeIf` sections by subsection, each with its (key, value) pairs.
fn sections(cwd: &Path, env: &[(&str, &str)]) -> Result<Sections, String> {
    let (out, bytes) =
        gitcli::output_bytes(cwd, &args(&["config", "--global", "-z", "--get-regexp", r"^includeif\."]), env, Some(gitcli::READ_TIMEOUT));
    if !out.ok {
        return if out.code == Some(1) { Ok(Vec::new()) } else { Err(format!("{SYNC_FAILED}: {}", out.combined())) };
    }
    Ok(group(&bytes))
}

/// `--get-regexp -z` output (`<key>\n<value>\0` each) grouped by subsection, in file order.
fn group(bytes: &[u8]) -> Sections {
    let mut out: Sections = Vec::new();
    for item in bytes.split(|b| *b == 0).filter(|i| !i.is_empty()) {
        let item = String::from_utf8_lossy(item);
        let (key, value) = item.split_once('\n').unwrap_or((&item, ""));
        let Some(rest) = key.strip_prefix("includeif.") else { continue };
        let Some((sub, name)) = rest.rsplit_once('.') else { continue };
        let pair = (name.to_owned(), value.to_owned());
        match out.iter_mut().find(|(s, _)| s == sub) {
            Some((_, keys)) => keys.push(pair),
            None => out.push((sub.to_owned(), vec![pair])),
        }
    }
    out
}

/// The rule for the folder at `path` (absolute), or why it can't have one: resolved on disk,
/// an existing folder named in valid text, and whether its file system folds case (probed by
/// asking for the folder in the other case). Touches the disk: worker threads only.
pub fn check_folder(path: &Path, repository: bool) -> Result<FolderRule, String> {
    if !path.is_absolute() {
        return Err("Choose a folder by its full path.".into());
    }
    let resolved = paths::resolve(path);
    if !resolved.is_dir() {
        return Err("That folder doesn't exist.".into());
    }
    if resolved.to_str().is_none() {
        return Err(NOT_TEXT.into());
    }
    let ignore_case = pattern::flip(&resolved).is_none_or(|f| paths::same_folder(&resolved, &f));
    let rule = FolderRule { path: paths::encode(&resolved), repository, ignore_case };
    gitdir_pattern(&rule)?;
    Ok(rule)
}

#[cfg(test)]
mod tests;
