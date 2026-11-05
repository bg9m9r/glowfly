//! The bundled catalogs: per language, one flat JSON object of key → text per area
//! (`locales/<lang>/<area>.json`, the area being every key's first segment), compiled in and
//! merged into one table on first use. The tests check that each file parses, holds only its
//! area's keys, and that no key is in two files.

use super::Lang;
use serde::de::{Deserializer, MapAccess, Visitor};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::OnceLock;

/// One language's area files, in `AREAS` order. A file missing from the folder fails the build.
macro_rules! area_files {
    ($code:literal, $($area:literal)*) => {
        &[$(include_str!(concat!("../../locales/", $code, "/", $area, ".json"))),*]
    };
}

/// `AREAS` and every language's files for them.
macro_rules! catalogs {
    ($($area:literal)*) => {
        /// Every area, sorted: the file stems of each `locales/<lang>/` folder.
        pub(super) const AREAS: &[&str] = &[$($area),*];
        const EN: &[&str] = area_files!("en", $($area)*);
        const DE: &[&str] = area_files!("de", $($area)*);
        const ES: &[&str] = area_files!("es", $($area)*);
    };
}

catalogs! {
    "about" "access" "auto_stash" "blame" "branch" "branch_push" "branch_sync" "central"
    "clone" "commands" "commit" "commits" "common" "config_errors" "conflict" "conflict_check"
    "credential_errors" "dashboard" "dates" "deep_link" "details" "dialog" "diff" "files"
    "finder" "forge" "frame_times" "getting_started" "git_banner" "git_errors" "gitflow" "glance"
    "graph" "hints" "history" "job" "job_retry" "journal" "layout" "lfs"
    "lock" "logs" "losses" "mainline" "markdown" "menu" "op" "op_preview"
    "op_strip" "palette" "panel" "patch" "pick" "pr" "pr_checkout" "prefs"
    "publish" "quit" "range_diff" "read_errors" "rebase" "rebase_errors" "refs" "release_notes" "remote_branch"
    "repo_colour" "repo_group" "repo_sync" "repos" "safe_dir" "search" "service_errors" "settings_file"
    "sidebar" "sign_in" "signature" "signing_errors" "stack" "stash" "status" "submodules"
    "sync" "tabs" "tag" "terminal" "title_bar" "toolbar" "tools" "tour"
    "trust" "undo" "update_errors" "updates" "watch" "welcome" "welcome_screen" "worktree_branch"
}

static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
static DE_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
static ES_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// `lang`'s texts by key.
pub(super) fn table(lang: Lang) -> &'static HashMap<String, String> {
    let cell = match lang {
        Lang::En => &EN_TABLE,
        Lang::De => &DE_TABLE,
        Lang::Es => &ES_TABLE,
    };
    cell.get_or_init(|| merge(files(lang)).0)
}

/// `lang`'s files as they are compiled in: (area, source).
pub(super) fn files(lang: Lang) -> impl Iterator<Item = (&'static str, &'static str)> {
    let sources = match lang {
        Lang::En => EN,
        Lang::De => DE,
        Lang::Es => ES,
    };
    AREAS.iter().copied().zip(sources.iter().copied())
}

/// A file's key/text pairs in file order, repeated keys kept, or none when it doesn't parse.
pub(super) fn entries(src: &str) -> Option<Vec<(String, String)>> {
    let mut de = serde_json::Deserializer::from_str(src);
    let pairs = (&mut de).deserialize_map(Pairs).ok()?;
    de.end().ok()?;
    Some(pairs)
}

/// The texts of `files` merged (the first of a repeated key wins) and every key seen more than
/// once. A file that doesn't parse is skipped (the tests make sure none does).
pub(super) fn merge<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> (HashMap<String, String>, Vec<String>) {
    let mut texts = HashMap::new();
    let mut dups = Vec::new();
    let mut reported = HashSet::new();
    for (_, src) in files {
        for (key, text) in entries(src).unwrap_or_default() {
            match texts.entry(key) {
                Entry::Vacant(slot) => {
                    slot.insert(text);
                }
                Entry::Occupied(slot) => {
                    if reported.insert(slot.key().clone()) {
                        dups.push(slot.key().clone());
                    }
                }
            }
        }
    }
    (texts, dups)
}

/// Reads a JSON object into its pairs in order, keeping repeated keys.
struct Pairs;

impl<'de> Visitor<'de> for Pairs {
    type Value = Vec<(String, String)>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object of key to text")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::new();
        while let Some(pair) = map.next_entry::<String, String>()? {
            out.push(pair);
        }
        Ok(out)
    }
}
