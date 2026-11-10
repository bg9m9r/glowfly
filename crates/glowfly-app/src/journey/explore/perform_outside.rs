//! Changes from outside the app: files written, appended, created and deleted in the work tree,
//! and git run on the command line. Each ends with Refresh, as a user would after switching back.
//! A write that fails (on Windows, a sharing violation) is tried once more after the app settles.

use super::super::find::{Pick, Region};
use super::super::fixture::git_try;
use super::super::harness::Journey;
use super::action::Outside;
use super::perform::Done;
use super::shadow::Shadow;
use std::path::Path;

/// Runs `op` and, when it fails, once more after the app settles.
fn twice<T>(j: &mut Journey, mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    op().or_else(|_| {
        j.settle();
        op()
    })
}

fn eol_of(text: &str, path: &str) -> &'static str {
    if text.contains("\r\n") || path == "crlf.txt" { "\r\n" } else { "\n" }
}

/// How an edit treats the file's old text.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Write,
    Append,
    Create,
}

pub(super) fn perform_outside(j: &mut Journey, dir: &Path, repo: &str, shadow: &mut Shadow, o: &Outside) -> Done {
    let done = match o {
        Outside::Write { path, lines } => edit(j, dir, repo, shadow, path, *lines, Kind::Write),
        Outside::Create { path, lines } => edit(j, dir, repo, shadow, path, *lines, Kind::Create),
        Outside::Append { path, lines } => edit(j, dir, repo, shadow, path, *lines, Kind::Append),
        Outside::Delete { path } => delete(j, dir, repo, shadow, path),
        Outside::GitCommit(m) => {
            let ok = git_try(dir, &["add", "-A"]) && git_try(dir, &["commit", "-q", "-m", m]);
            if ok {
                shadow.summary(repo, m.lines().next().unwrap_or_default());
            }
            git_done(ok, "git commit")
        }
        Outside::GitCheckout(b) => git_done(git_try(dir, &["checkout", "-q", b]), "git checkout"),
        Outside::GitBranch(b) => git_done(git_try(dir, &["branch", b]), "git branch"),
    };
    j.try_click("Refresh the repository", true, Region::Toolbar, Pick::First);
    done
}

fn git_done(ok: bool, what: &str) -> Done {
    if ok { Done::Performed } else { Done::Skipped(format!("{what} failed")) }
}

fn edit(j: &mut Journey, dir: &Path, repo: &str, shadow: &mut Shadow, path: &str, count: u8, kind: Kind) -> Done {
    let file = dir.join(path);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    if kind == Kind::Create && file.exists() {
        return Done::Skipped(format!("{path} exists"));
    }
    let eol = eol_of(&old, path);
    let markers: Vec<String> = (0..count).map(|_| shadow.marker()).collect();
    let added: String = markers.iter().map(|m| format!("{m}{eol}")).collect();
    let separator = if kind == Kind::Append && !old.is_empty() && !old.ends_with('\n') { eol } else { "" };
    let text = if kind == Kind::Append { format!("{old}{separator}{added}") } else { added };
    let written = twice(j, || {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file, &text)
    });
    if let Err(e) = written {
        return Done::Skipped(format!("could not write {path}: {e}"));
    }
    if kind != Kind::Append {
        shadow.forget_in(repo, &old);
    }
    for m in markers {
        shadow.add(repo, m);
    }
    Done::Performed
}

fn delete(j: &mut Journey, dir: &Path, repo: &str, shadow: &mut Shadow, path: &str) -> Done {
    let file = dir.join(path);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    match twice(j, || std::fs::remove_file(&file)) {
        Ok(()) => {
            shadow.forget_in(repo, &old);
            Done::Performed
        }
        Err(e) => Done::Skipped(format!("could not delete {path}: {e}")),
    }
}
