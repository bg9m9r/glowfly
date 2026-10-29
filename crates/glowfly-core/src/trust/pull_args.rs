//! Reading a `git pull` command's arguments (`Pull::of`): which go to its fetch, which to the
//! merge or rebase after it, and the remote and refspec it names.

use super::Pull;

/// Options that go to both commands.
const BOTH: &[&str] = &["-q", "--quiet", "-v", "--verbose", "--progress", "--no-progress"];
/// Options that go to the merge only.
const MERGE: &[&str] = &[
    "--commit",
    "--no-commit",
    "-e",
    "--edit",
    "--no-edit",
    "--stat",
    "-n",
    "--no-stat",
    "--summary",
    "--no-summary",
    "--compact-summary",
    "--squash",
    "--no-squash",
    "--verify-signatures",
    "--no-verify-signatures",
    "--allow-unrelated-histories",
];
/// Options that go to the merge and to the rebase.
const SHARED: &[&str] = &["--signoff", "--no-signoff", "--verify", "--no-verify", "--autostash", "--no-autostash", "--no-gpg-sign"];
/// Options of the merge and the rebase that take the next argument as their value.
const SHARED_VALUED: &[&str] = &["-s", "--strategy", "-X", "--strategy-option"];
/// Fetch options that take the next argument as their value.
const FETCH_VALUED: &[&str] = &[
    "--depth",
    "--deepen",
    "--shallow-since",
    "--shallow-exclude",
    "-j",
    "--jobs",
    "--upload-pack",
    "-o",
    "--server-option",
    "--refmap",
    "--negotiation-tip",
    "--filter",
];

impl Pull {
    /// The pull `cmd` (its arguments, without `git`), or None when it isn't one.
    pub fn of(cmd: &[String]) -> Option<Pull> {
        let at = crate::trust::incoming::sub_at(cmd)?;
        if cmd[at] != "pull" {
            return None;
        }
        let globals = cmd[..at].to_vec();
        let mut p = Pull {
            globals: globals.clone(),
            fetch: [globals, vec!["fetch".to_owned(), "--update-head-ok".to_owned()]].concat(),
            merge: Vec::new(),
            rebase: Vec::new(),
            how: None,
            ff: None,
            dry_run: false,
            remote: None,
            refspec: None,
            recurse: None,
            cmd: cmd.to_vec(),
            reflog: std::iter::once("pull").chain(cmd[at + 1..].iter().map(String::as_str)).collect::<Vec<_>>().join(" "),
        };
        let mut it = cmd[at + 1..].iter();
        while let Some(a) = it.next() {
            let s = a.as_str();
            match s {
                "-r" | "--rebase" => p.how = Some("true".to_owned()),
                "--no-rebase" => p.how = Some("false".to_owned()),
                _ if s.starts_with("--rebase=") => p.how = Some(s["--rebase=".len()..].to_owned()),
                "--ff" | "--no-ff" | "--ff-only" => p.ff = Some(s.to_owned()),
                "--dry-run" => {
                    p.dry_run = true;
                    p.fetch.push(a.clone());
                }
                _ if BOTH.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                _ if MERGE.contains(&s) || s.starts_with("--cleanup=") || s == "--no-log" || s.starts_with("--log") => {
                    p.merge.push(a.clone())
                }
                "--no-recurse-submodules" => {
                    p.recurse = Some(false);
                    p.fetch.push(a.clone());
                }
                _ if s.starts_with("--recurse-submodules") => {
                    p.recurse = Some(!s.ends_with("=no"));
                    p.fetch.push(a.clone());
                }
                _ if SHARED.contains(&s) || s.starts_with("-S") || s.starts_with("--gpg-sign") || shared_joined(s) => {
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                _ if SHARED_VALUED.contains(&s) => {
                    let v = it.next().cloned().unwrap_or_default();
                    p.merge.extend([a.clone(), v.clone()]);
                    p.rebase.extend([a.clone(), v]);
                }
                _ if FETCH_VALUED.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.fetch.extend(it.next().cloned());
                }
                _ if s.starts_with('-') => p.fetch.push(a.clone()),
                _ => {
                    if p.remote.is_none() {
                        p.remote = Some(a.clone());
                    } else if p.refspec.is_none() {
                        p.refspec = Some(a.clone());
                    }
                    p.fetch.push(a.clone());
                }
            }
        }
        Some(p)
    }
}

/// `-s<x>`, `-X<x>` or `--strategy=<x>` written as one argument.
fn shared_joined(s: &str) -> bool {
    (s.len() > 2 && (s.starts_with("-s") || s.starts_with("-X"))) || s.starts_with("--strategy=") || s.starts_with("--strategy-option=")
}
