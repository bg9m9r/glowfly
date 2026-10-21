//! Custom commands as loaded (the user's and the open repository's commands files), what their
//! placeholders hold where they run, their keys, the words for their problems and the items of
//! the menus' Commands submenu. Pure rules and data; loading and running are in
//! update/custom_commands*.rs.

use super::AppState;
use super::trust::Standing;
use crate::custom_commands::{CommandRef, CustomCommandMsg, Target};
use crate::keymap::{Chord, Keymap, binding};
use git2::Oid;
use glowfly_core::custom_commands::{Command, MAX_COMMANDS, MAX_NAME_CHARS, Missing, Problem, ProblemKind, Source, Vars};
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::Snapshot;
use std::path::{Path, PathBuf};

/// A key a command claims: its chord, the command and the chord's text for hints.
pub type CommandKey = (Chord, CommandRef, String);

#[derive(Default)]
pub struct CustomCommands {
    /// From the user's own file.
    pub user: Vec<Command>,
    /// From the repository's `.glowfly/commands.toml`.
    pub repo: Vec<Command>,
    /// The working folder `repo` was read for.
    pub repo_for: Option<PathBuf>,
    /// The latest load asked for; an answer to an older one is dropped.
    pub generation: u64,
    pub keys: Vec<CommandKey>,
    /// The problem texts last announced, so the same ones aren't announced on every load.
    pub reported: Vec<String>,
}

impl CustomCommands {
    /// The commands offered in the repository at `workdir`: the user's, then the repository's
    /// (only when they were read for that folder), in file order.
    pub fn listed(&self, workdir: Option<&Path>) -> impl Iterator<Item = (Source, &Command)> {
        let open = workdir.is_some();
        let here = workdir.is_some_and(|w| self.repo_for.as_deref().is_some_and(|r| same_spelling(r, w)));
        let user = self.user.iter().filter(move |_| open).map(|c| (Source::User, c));
        let repo = self.repo.iter().filter(move |_| here).map(|c| (Source::Repo, c));
        user.chain(repo)
    }

    /// The command `r` names, if it is still offered at `workdir`.
    pub fn find(&self, r: &CommandRef, workdir: Option<&Path>) -> Option<&Command> {
        self.listed(workdir).find(|(s, c)| *s == r.source && c.name == r.name).map(|(_, c)| c)
    }

    /// The key text of a palette entry that runs a custom command with a key.
    pub fn key_hint(&self, cmd: &super::palette::PaletteCommand) -> Option<&str> {
        let super::palette::PaletteCommand::Custom(CustomCommandMsg::Run { command, .. }) = cmd else { return None };
        self.keys.iter().find(|(_, r, _)| r == command).map(|(_, _, hint)| hint.as_str())
    }
}

/// What a command's placeholders hold, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedVars {
    pub repo: PathBuf,
    pub branch: Option<String>,
    pub sha: Option<String>,
    pub files: Vec<String>,
}

impl OwnedVars {
    pub fn as_vars(&self) -> Vars<'_> {
        Vars { repo: &self.repo, branch: self.branch.as_deref(), sha: self.sha.as_deref(), files: &self.files }
    }
}

/// What the placeholders hold for a command run from `at` in `snap`; for the palette,
/// `selected` is the single selected commit and `open_file` the file whose diff is open.
pub fn vars_for(snap: &Snapshot, at: &Target, selected: Option<Oid>, open_file: Option<&str>) -> OwnedVars {
    let head = snap.head_oid.map(|o| o.to_string());
    let mut v = OwnedVars { repo: snap.workdir.clone(), branch: snap.head_branch.clone(), sha: head, files: Vec::new() };
    match at {
        Target::Palette => {
            if let Some(o) = selected {
                v.sha = Some(o.to_string());
            }
            v.files.extend(open_file.map(str::to_owned));
        }
        Target::Commit(oid) => v.sha = Some(oid.to_string()),
        Target::Branch(name) => {
            v.branch = Some(name.clone());
            v.sha = snap.locals.iter().find(|b| &b.name == name).and_then(|b| b.oid).map(|o| o.to_string());
        }
        Target::File { path, at } => {
            v.files.push(path.clone());
            if let Some(o) = at {
                v.sha = Some(o.to_string());
            }
        }
    }
    v
}

/// Why a command can't run here.
pub fn missing_text(t: Lang, m: Missing) -> &'static str {
    match m {
        Missing::Branch => t.tr("commands.missing.branch"),
        Missing::Sha => t.tr("commands.missing.sha"),
        Missing::Files => t.tr("commands.missing.files"),
        Missing::Repo => t.tr("commands.missing.repo"),
    }
}

/// The palette's detail for a command from `source`; `held`: it asks to trust the repository
/// first.
pub fn source_detail(t: Lang, source: Source, held: bool) -> String {
    match (source, held) {
        (Source::User, _) => t.tr("commands.palette.user"),
        (Source::Repo, false) => t.tr("commands.palette.repo"),
        (Source::Repo, true) => t.tr("commands.palette.held"),
    }
    .to_owned()
}

/// Whether the active repository's own commands may run without asking: it is Trusted.
pub fn repo_trusted(st: &AppState) -> bool {
    st.trust_standing() == Some(Standing::Trusted)
}

/// A problem of the commands file `file` (as shown) for the activity log.
pub fn problem_text(t: Lang, file: &str, p: &Problem) -> String {
    let text = kind_text(t, &p.kind);
    match p.line {
        Some(line) => t.trf("commands.problem.at_line", &[("file", &file), ("line", &line), ("text", &text)]),
        None => t.trf("commands.problem.in_file", &[("file", &file), ("text", &text)]),
    }
}

fn kind_text(t: Lang, k: &ProblemKind) -> String {
    match k {
        ProblemKind::Syntax(m) => t.trf("commands.problem.syntax", &[("message", m)]),
        ProblemKind::TooLarge => t.tr("commands.problem.too_large").to_owned(),
        ProblemKind::NotUtf8 => t.tr("commands.problem.not_utf8").to_owned(),
        ProblemKind::Unreadable(e) => t.trf("commands.problem.unreadable", &[("error", e)]),
        ProblemKind::Link => t.tr("commands.problem.link").to_owned(),
        ProblemKind::TooMany => t.trf("commands.problem.too_many", &[("max", &MAX_COMMANDS)]),
        ProblemKind::EmptyName => t.tr("commands.problem.empty_name").to_owned(),
        ProblemKind::LongName => t.trf("commands.problem.long_name", &[("max", &MAX_NAME_CHARS)]),
        ProblemKind::DuplicateName(n) => t.trf("commands.problem.duplicate", &[("name", n)]),
        ProblemKind::EmptyTemplate => t.tr("commands.problem.empty_template").to_owned(),
        ProblemKind::UnmatchedQuote => t.tr("commands.problem.unmatched_quote").to_owned(),
        ProblemKind::UnknownPlaceholder(n) => {
            let known = glowfly_core::custom_commands::KNOWN.join(", ");
            t.trf("commands.problem.unknown_placeholder", &[("name", n), ("known", &known)])
        }
        ProblemKind::FilesNotAlone => t.trf("commands.problem.files_alone", &[("name", &"{files}")]),
        ProblemKind::QuotedPlaceholder(n) => t.trf("commands.problem.quoted", &[("name", n)]),
        ProblemKind::NoPlaces => t.tr("commands.problem.no_places").to_owned(),
    }
}

/// The keys `lists` (each file's source, its name as shown and its commands) claim, and the
/// problems of the keys that can't be used: not a chord, no Mod, Ctrl or Alt, a built-in
/// binding's key, or a key an earlier command claimed.
pub fn chords(t: Lang, keymap: &Keymap, lists: &[(Source, &str, &[Command])]) -> (Vec<CommandKey>, Vec<String>) {
    let mut keys: Vec<CommandKey> = Vec::new();
    let mut problems = Vec::new();
    let fold = !cfg!(target_os = "macos");
    for (source, file, commands) in lists {
        for c in commands.iter() {
            let Some(text) = c.key.as_deref() else { continue };
            let at = |what: String| t.trf("commands.problem.at_line", &[("file", file), ("line", &c.key_line), ("text", &what)]);
            let Some(chord) = Chord::parse(text) else {
                problems.push(at(t.trf("commands.problem.bad_key", &[("key", &text)])));
                continue;
            };
            let shown = chord.text();
            if !chord.has_command_mod() {
                problems.push(at(t.trf("commands.problem.key_modifier", &[("key", &shown)])));
                continue;
            }
            if let Some(id) = keymap.lookup(chord, fold) {
                problems.push(at(t.trf("commands.problem.key_taken", &[("key", &shown), ("action", &binding(id).label)])));
                continue;
            }
            if let Some((_, other, _)) = keys.iter().find(|(k, _, _)| crate::keymap::same_keys(*k, chord, fold)) {
                problems.push(at(t.trf("commands.problem.key_taken", &[("key", &shown), ("action", &other.name)])));
                continue;
            }
            keys.push((chord, CommandRef { source: *source, name: c.name.clone() }, shown));
        }
    }
    (keys, problems)
}

#[path = "custom_menu_items.rs"]
mod menu_items;
pub use menu_items::MenuCommands;
