//! The open repositories' effective identities as git reported them (`AppState::identities`, by
//! git dir), the include sync's state (`AppState::includes`), and the words built from them: the
//! Profile page's Open repository row, the commit box's identity line and the Repo > Identity
//! hints, plus the warnings both views give when `GIT_AUTHOR_*` or `GIT_COMMITTER_*` in
//! Glowfly's environment take the place of the config (`RepoIdentity::overrides`). Pure.

use crate::state::profiles::identity_line;
use glowfly_core::i18n::Lang;
use glowfly_core::repo_identity::{IdentitySource, RepoIdentity};
use glowfly_core::settings::{ProfileId, Settings, rule_for_git_dir, shown_folder};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Writing the include files and the global entries: one sync at a time, never during a
/// profile switch (they share the global config's lock).
#[derive(Default)]
pub struct IncludeSync {
    pub running: bool,
    /// A sync asked for while one ran (or a switch did): whether any of those asked to place
    /// the entries.
    pub again: Option<bool>,
    /// A switch chosen while a sync ran: it is already busy (`AppState::profile_switch`), and
    /// its writes start when the sync ends.
    pub held_switch: Option<ProfileId>,
    /// The git dirs whose identity disagreed with their rule and got a placing sync this
    /// session (at most one each).
    pub healed: HashSet<PathBuf>,
    /// The last sync's failure.
    pub error: Option<String>,
}

const NO_IDENTITY_HERE: &str = "No author identity set.";

fn who(id: &RepoIdentity) -> Option<String> {
    identity_line(&id.name, &id.email)
}

/// The repository's own name and email, as one line.
pub fn local_who(id: &RepoIdentity) -> String {
    identity_line(id.local_name.as_deref().unwrap_or(""), id.local_email.as_deref().unwrap_or("")).unwrap_or_default()
}

fn profile_name(settings: &Settings, id: ProfileId) -> String {
    settings.profile(id).map_or_else(|| "(deleted)".to_owned(), |p| p.name.clone())
}

/// The Open repository row: where the identity of the repository `repo` (git dir `git_dir`)
/// comes from.
pub fn here_line(repo: &str, id: &RepoIdentity, settings: &Settings, git_dir: &Path) -> String {
    let Some(who) = who(id) else { return NO_IDENTITY_HERE.into() };
    match &id.source {
        IdentitySource::Profile(p) => {
            let rules: Vec<_> = settings.rules().into_iter().filter(|(owner, _)| owner == p).collect();
            let name = profile_name(settings, *p);
            match rule_for_git_dir(&rules, git_dir) {
                Some((_, rule)) => {
                    let folder = glowfly_core::native_path::shown(&shown_folder(rule));
                    format!("{repo} commits as {who}, from the profile {name} (folder rule {folder}).")
                }
                None => format!("{repo} commits as {who}, from the profile {name}."),
            }
        }
        IdentitySource::Repository => format!("{repo} sets its own identity in its Git config: {who}."),
        IdentitySource::Elsewhere => match settings.active_profile() {
            Some(p) => format!("{repo} commits as {who}, your global identity (profile {}).", p.name),
            None => format!("{repo} commits as {who}, your global identity."),
        },
        IdentitySource::Unset => NO_IDENTITY_HERE.into(),
    }
}

/// The author a commit gets: `GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL` in place of the config's.
fn author_who(id: &RepoIdentity) -> Option<String> {
    let name = id.override_of("GIT_AUTHOR_NAME").unwrap_or(&id.name);
    let email = id.override_of("GIT_AUTHOR_EMAIL").unwrap_or(&id.email);
    identity_line(name, email)
}

/// The identity variables that are set, by name, for a line.
fn override_names(id: &RepoIdentity) -> String {
    id.overrides.iter().map(|o| o.var).collect::<Vec<_>>().join(", ")
}

/// The first name variable set but empty: git refuses to commit with an empty name
/// (`empty ident name not allowed`). An empty email is allowed.
fn empty_name(id: &RepoIdentity) -> Option<&'static str> {
    id.overrides.iter().find(|o| o.var.ends_with("_NAME") && o.value.is_empty()).map(|o| o.var)
}

/// The commit box's identity line and whether it is a warning (no identity, or variables in the
/// environment take the place of the config; an empty name variable, which stops commits).
pub fn commit_identity_line(t: Lang, id: &RepoIdentity, settings: &Settings) -> (String, bool) {
    if let Some(var) = empty_name(id) {
        return (t.trf("commit.identity.env.empty_name", &[("var", &var)]), true);
    }
    if !id.overrides.is_empty() {
        let line = match author_who(id) {
            Some(who) => t.trf("commit.identity.env.line", &[("who", &who), ("vars", &override_names(id))]),
            None => t.tr("commit.identity.unset").to_owned(),
        };
        return (line, true);
    }
    let Some(who) = who(id) else { return (t.tr("commit.identity.unset").to_owned(), true) };
    let with_profile = |name: &str| t.trf("commit.identity.profile", &[("who", &who), ("profile", &name)]);
    let text = match &id.source {
        IdentitySource::Profile(p) => with_profile(&profile_name(settings, *p)),
        IdentitySource::Repository => t.trf("commit.identity.repo", &[("who", &who)]),
        IdentitySource::Elsewhere | IdentitySource::Unset => match settings.active_profile() {
            Some(p) => with_profile(&p.name),
            None => t.trf("commit.identity.plain", &[("who", &who)]),
        },
    };
    (text, false)
}

/// The commit box's identity line's hover text while variables in the environment take the
/// place of the config; None when none is set.
pub fn env_tip(t: Lang, id: &RepoIdentity) -> Option<String> {
    let set: Vec<String> = id.overrides.iter().map(|o| format!("{}={}", o.var, shown_value(&o.value))).collect();
    (!set.is_empty()).then(|| t.trf("commit.identity.env.tip", &[("vars", &set.join(", "))]))
}

/// A variable's value for a line, without control or bidi characters (`text::printable`), so it
/// can't break or reorder the line; an empty one as `""`, which git takes too.
fn shown_value(value: &str) -> String {
    let shown = glowfly_core::text::printable(value);
    if shown.is_empty() { "\"\"".to_owned() } else { shown }
}

/// The Profile page's Open repository warnings: one per identity variable set in the
/// environment, naming it and the value commits take from it (or, for an empty name, that git
/// won't commit).
pub fn env_warnings(t: Lang, id: &RepoIdentity) -> Vec<String> {
    let line = |var: &str, value: &str| {
        if var.ends_with("_NAME") && value.is_empty() {
            return t.trf("prefs.profile.env.empty_name.warn", &[("var", &var)]);
        }
        let value = shown_value(value);
        let args: [(&str, &dyn std::fmt::Display); 2] = [("var", &var), ("value", &value)];
        match var {
            "GIT_AUTHOR_NAME" => t.trf("prefs.profile.env.author_name.warn", &args),
            "GIT_AUTHOR_EMAIL" => t.trf("prefs.profile.env.author_email.warn", &args),
            "GIT_COMMITTER_NAME" => t.trf("prefs.profile.env.committer_name.warn", &args),
            _ => t.trf("prefs.profile.env.committer_email.warn", &args),
        }
    };
    id.overrides.iter().map(|o| line(o.var, &o.value)).collect()
}

/// The profile a rule says applies, when the repository's own identity comes first and
/// differs from it (the Open repository row's warning and Use <profile> here).
pub fn shadowed(id: &RepoIdentity, settings: &Settings) -> Option<ProfileId> {
    let p = id.expected?;
    if id.source != IdentitySource::Repository {
        return None;
    }
    let profile = settings.profile(p)?;
    let differs = |local: &Option<String>, mine: &str| local.as_deref().is_some_and(|v| v.trim() != mine.trim());
    (differs(&id.local_name, &profile.author_name) || differs(&id.local_email, &profile.author_email)).then_some(p)
}

/// The profile whose rule should apply but which git didn't give (the repository sets nothing
/// itself, and the profile has an email to give): what a placing sync may repair.
pub fn needs_repair(id: &RepoIdentity, settings: &Settings) -> Option<ProfileId> {
    let p = id.expected?;
    let gave = id.source == IdentitySource::Profile(p) || id.source == IdentitySource::Repository;
    let email = settings.profile(p).is_some_and(|pr| !pr.author_email.trim().is_empty());
    (!gave && email).then_some(p)
}

/// Whether git still doesn't apply the rule after its one repair.
pub fn not_applied(id: &RepoIdentity, settings: &Settings, healed: bool) -> bool {
    healed && needs_repair(id, settings).is_some()
}

pub fn not_applied_line(name: &str) -> String {
    format!("Git doesn't apply the {name} folder rule here (for example, a linked worktree whose repository is outside the folder).")
}

pub fn shadow_line(name: &str) -> String {
    format!("This comes before the {name} folder rule.")
}

pub fn use_here_label(name: &str) -> String {
    format!("Use {name} here")
}

pub const USE_HERE_TITLE: &str = "Use profile here";

pub fn use_here_body(local: &str, name: &str) -> String {
    format!(
        "Remove this repository's own identity ({local}) from its Git config so the profile {name} applies? Its user.name and user.email are unset; a signing key set there stays."
    )
}

pub fn use_here_button(name: &str) -> String {
    format!("Remove and use {name}")
}

/// Repo > Identity's Name and Email hints: what the repository gets without its own values (a
/// profile's rule, else the global identity).
pub fn inherit_hints(id: Option<&RepoIdentity>, settings: &Settings, global_name: &str, global_email: &str) -> (String, String) {
    let hint = |value: &str, from: Option<&str>| match (value.trim(), from) {
        ("", _) => "not set".to_owned(),
        (v, Some(name)) => format!("inherits {v} (profile {name})"),
        (v, None) => format!("inherits {v}"),
    };
    let profile = |p: ProfileId| settings.profile(p).map(|pr| (pr.author_name.clone(), pr.author_email.clone(), pr.name.clone()));
    let (name, email, from) = match id.map(|i| (&i.source, i)) {
        Some((IdentitySource::Profile(p), i)) => (i.name.clone(), i.email.clone(), Some(profile_name(settings, *p))),
        Some((IdentitySource::Elsewhere, i)) => (i.name.clone(), i.email.clone(), None),
        Some((IdentitySource::Unset, _)) => (String::new(), String::new(), None),
        Some((IdentitySource::Repository, i)) => match i.expected.and_then(profile) {
            Some((n, e, p)) => (n, e, Some(p)),
            None => (global_name.to_owned(), global_email.to_owned(), None),
        },
        None => (global_name.to_owned(), global_email.to_owned(), None),
    };
    (hint(&name, from.as_deref()), hint(&email, from.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Profile;

    const WORK: ProfileId = ProfileId(1);

    fn settings() -> Settings {
        let work = Profile {
            id: WORK,
            name: "Work".into(),
            author_name: "Ada".into(),
            author_email: "ada@work.example".into(),
            ..Profile::default()
        };
        Settings { profiles: vec![work], active_profile: Some(WORK), ..Settings::default() }
    }

    fn read(source: IdentitySource, expected: Option<ProfileId>) -> RepoIdentity {
        RepoIdentity { name: "Ada".into(), email: "x@y.example".into(), source, expected, ..RepoIdentity::default() }
    }

    #[test]
    fn lines_follow_the_source() {
        let s = settings();
        let t = Lang::En;
        let git_dir = Path::new("unused");
        assert_eq!(
            here_line("acme", &read(IdentitySource::Profile(WORK), None), &s, git_dir),
            "acme commits as Ada <x@y.example>, from the profile Work."
        );
        assert_eq!(
            here_line("acme", &read(IdentitySource::Repository, None), &s, git_dir),
            "acme sets its own identity in its Git config: Ada <x@y.example>."
        );
        assert!(here_line("acme", &read(IdentitySource::Elsewhere, None), &s, git_dir).ends_with("your global identity (profile Work)."));
        assert_eq!(here_line("acme", &RepoIdentity::default(), &s, git_dir), NO_IDENTITY_HERE);
        assert_eq!(
            commit_identity_line(t, &read(IdentitySource::Profile(WORK), None), &s),
            ("as Ada <x@y.example> · Work".to_owned(), false)
        );
        assert_eq!(commit_identity_line(t, &read(IdentitySource::Repository, None), &s).0, "as Ada <x@y.example> · this repository");
        assert_eq!(commit_identity_line(t, &RepoIdentity::default(), &s), ("No author identity".to_owned(), true));
        let none = Settings::default();
        assert_eq!(commit_identity_line(t, &read(IdentitySource::Elsewhere, None), &none).0, "as Ada <x@y.example>");
    }

    #[test]
    fn shadowing_and_repairs() {
        let s = settings();
        let mut local = read(IdentitySource::Repository, Some(WORK));
        local.local_email = Some("x@y.example".into());
        assert_eq!(shadowed(&local, &s), Some(WORK));
        assert_eq!(needs_repair(&local, &s), None, "the repository's own identity is the user's");
        local.local_email = Some("ada@work.example".into());
        assert_eq!(shadowed(&local, &s), None, "the same values");
        let global = read(IdentitySource::Elsewhere, Some(WORK));
        assert_eq!(needs_repair(&global, &s), Some(WORK));
        assert!(!not_applied(&global, &s, false) && not_applied(&global, &s, true));
        assert_eq!(needs_repair(&read(IdentitySource::Profile(WORK), Some(WORK)), &s), None);
    }

    #[test]
    fn hints_name_the_profile() {
        let s = settings();
        let ruled = RepoIdentity {
            name: "Ada".into(),
            email: "ada@work.example".into(),
            source: IdentitySource::Profile(WORK),
            ..RepoIdentity::default()
        };
        assert_eq!(inherit_hints(Some(&ruled), &s, "G", "g@x").0, "inherits Ada (profile Work)");
        assert_eq!(inherit_hints(None, &s, "G", "").1, "not set");
        let own = RepoIdentity { source: IdentitySource::Repository, expected: Some(WORK), ..ruled.clone() };
        assert_eq!(inherit_hints(Some(&own), &s, "G", "g@x").1, "inherits ada@work.example (profile Work)");
    }
}
