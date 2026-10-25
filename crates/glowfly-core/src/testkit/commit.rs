//! The kit's in-process `git commit`: the plain forms, with the identity, dates, message
//! clean-up and reflog messages git uses.

use super::guard::{self, IDENTITY_ENV};
use git2::{ErrorCode, Repository, Signature, Time};

/// What `git commit` was asked to do.
#[derive(Default)]
struct Ask<'a> {
    all: bool,
    allow_empty: bool,
    amend: bool,
    no_edit: bool,
    messages: Vec<&'a str>,
    /// `--author "Name <email>"`.
    author: Option<(&'a str, &'a str)>,
}

fn parse<'a>(a: &[&'a str]) -> Option<Ask<'a>> {
    let mut ask = Ask::default();
    let mut it = a.iter();
    while let Some(&arg) = it.next() {
        match arg {
            "-q" | "--quiet" | "--no-verify" | "-n" => {}
            "-a" | "--all" => ask.all = true,
            "--allow-empty" => ask.allow_empty = true,
            "--amend" => ask.amend = true,
            "--no-edit" => ask.no_edit = true,
            "-m" | "--message" | "-am" | "-qam" => {
                ask.all |= arg.ends_with("am");
                ask.messages.push(it.next()?);
            }
            "--author" => {
                let (name, email) = it.next()?.split_once(" <")?;
                ask.author = Some((name, email.strip_suffix('>')?));
            }
            _ => return None,
        }
    }
    Some(ask)
}

/// `git commit [-q] [-a] [--allow-empty] [--amend] [--no-edit] [--author <who>] -m <msg>...`,
/// with `config` standing in for `-c user.name=... -c user.email=...` before the command.
pub(super) fn commit(repo: &Repository, a: &[&str], env: &[(&str, &str)], config: &[(&str, &str)]) -> bool {
    let Some(ask) = parse(a) else { return false };
    let config_says = |key: &str| repo.config().and_then(|c| c.get_bool(key)).unwrap_or(false);
    if config_says("commit.gpgsign") || (ask.amend && ask.author.is_some()) {
        return false;
    }
    let head = match repo.head() {
        Ok(h) => h.peel_to_commit().ok(),
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => None,
        Err(_) => return false,
    };
    if ask.amend && head.is_none() {
        return false;
    }
    let message = match (ask.messages.is_empty(), &head) {
        (true, Some(h)) if ask.amend && ask.no_edit => h.message_raw().unwrap_or("").to_owned(),
        (true, _) => return false,
        (false, _) => match git2::message_prettify(ask.messages.join("\n\n"), None) {
            Ok(m) if !m.trim().is_empty() => m,
            _ => return false,
        },
    };
    let Ok(mut index) = repo.index() else { return false };
    if index.has_conflicts() || (ask.all && index.update_all(["*"], None).is_err()) || guard::index_needs_cli(repo, &index) {
        return false;
    }
    let Ok(tree_id) = index.write_tree() else { return false };
    // Git refuses a commit that changes nothing, and an amend that would leave the commit with
    // its parent's tree.
    let base = match &head {
        Some(h) if ask.amend => h.parent(0).ok().map(|p| p.tree_id()),
        Some(h) => Some(h.tree_id()),
        None => None,
    };
    if base.map_or(index.is_empty(), |b| b == tree_id) && !ask.allow_empty {
        return false;
    }
    let author = signature(repo, "AUTHOR", env, config, ask.author);
    let (Some(author), Some(committer)) = (author, signature(repo, "COMMITTER", env, config, None)) else { return false };
    let Ok(tree) = repo.find_tree(tree_id) else { return false };
    if ask.all && index.write().is_err() {
        return false;
    }
    let made = match &head {
        // An amend keeps the author, as git does even with GIT_AUTHOR_* set; libgit2 would log
        // it as a plain commit, so the ref moves here with git's message.
        Some(h) if ask.amend => h.amend(None, None, Some(&committer), None, Some(&message), Some(&tree)).and_then(|id| {
            let summary = message.lines().next().unwrap_or("");
            repo.head()?.set_target(id, &format!("commit (amend): {summary}"))?;
            Ok(id)
        }),
        Some(h) => repo.commit(Some("HEAD"), &author, &committer, &message, &tree, &[h]),
        None => repo.commit(Some("HEAD"), &author, &committer, &message, &tree, &[]),
    };
    match made {
        Ok(_) => true,
        Err(e) => panic!("in-process commit failed after staging: {e}"),
    }
}

/// The author or committer (`who`) git would record: `GIT_<who>_NAME`, `_EMAIL` and `_DATE`
/// from `env` or the process, else `--author` (`given`), else `user.name` and `user.email`
/// from `config` overrides or the repository's config, and now. None when git would stop (no
/// identity) or the date is in a form this doesn't read.
fn signature(
    repo: &Repository,
    who: &str,
    env: &[(&str, &str)],
    config: &[(&str, &str)],
    given: Option<(&str, &str)>,
) -> Option<Signature<'static>> {
    let var = |part: &str| {
        let key = format!("GIT_{who}_{part}");
        debug_assert!(IDENTITY_ENV.contains(&key.as_str()));
        env.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned()).or_else(|| std::env::var(&key).ok())
    };
    let repo_config = repo.config().ok()?;
    let setting = |key: &str| {
        let over = config.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned());
        over.or_else(|| repo_config.get_string(key).ok())
    };
    let (name, email) = match given {
        Some((n, e)) => (n.to_owned(), e.to_owned()),
        None => (var("NAME").or_else(|| setting("user.name"))?, var("EMAIL").or_else(|| setting("user.email"))?),
    };
    match var("DATE") {
        None => Signature::now(&name, &email).ok(),
        Some(date) => Signature::new(&name, &email, &parse_date(&date)?).ok(),
    }
}

/// A git date in the forms the tests use: RFC 3339 (`2026-01-05T10:00:00+00:00`, `...Z`) or
/// seconds with an offset (`1600000000 +0000`, optionally with a leading `@`).
pub(super) fn parse_date(s: &str) -> Option<Time> {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s.trim()) {
        return Some(Time::new(t.timestamp(), t.offset().local_minus_utc() / 60));
    }
    let (secs, zone) = s.trim().trim_start_matches('@').split_once(' ')?;
    let secs: i64 = secs.parse().ok()?;
    let (sign, digits) = match zone.as_bytes().first()? {
        b'+' => (1, &zone[1..]),
        b'-' => (-1, &zone[1..]),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let minutes = digits[..2].parse::<i32>().ok()? * 60 + digits[2..].parse::<i32>().ok()?;
    Some(Time::new(secs, sign * minutes))
}
