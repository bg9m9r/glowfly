//! When a repository last fetched: the time of `FETCH_HEAD`, which every fetch and pull
//! rewrites. A linked worktree has its own `FETCH_HEAD` in its git dir, but a fetch from any
//! worktree also moves the shared remote-tracking refs, so the newer of the worktree's file and
//! the common dir's counts. Which remotes that fetch covered comes from the file's lines, each
//! ending in the URL git fetched from (`fetched_remotes`).

use super::RemoteInfo;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The newer `FETCH_HEAD` time of `git_dir` and `common` in Unix seconds; None when neither
/// file exists or its time is before the epoch.
pub(super) fn fetched_at(git_dir: &Path, common: &Path) -> Option<i64> {
    [git_dir, common].into_iter().filter_map(|dir| modified(&dir.join("FETCH_HEAD"))).max()
}

/// The remotes the newer `FETCH_HEAD` of `git_dir` and `common` names, in `remotes` order: those
/// whose fetch URL (raw, or as libgit2 rewrote it in `rewritten`) a line of it ends in. Empty
/// when there is no file or it names none of them.
pub(super) fn fetched_remotes(git_dir: &Path, common: &Path, remotes: &[RemoteInfo], rewritten: &[(String, String)]) -> Vec<String> {
    let newest =
        [git_dir, common].into_iter().map(|d| d.join("FETCH_HEAD")).filter_map(|f| Some((modified(&f)?, f))).max_by_key(|(t, _)| *t);
    let Some((_, file)): Option<(i64, PathBuf)> = newest else { return Vec::new() };
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let urls: Vec<String> = text.lines().filter_map(line_url).map(|u| bare_url(&u)).collect();
    let named = |r: &RemoteInfo| {
        let own = r.urls.iter().chain(rewritten.iter().filter(|(n, _)| *n == r.name).map(|(_, u)| u));
        own.map(|u| bare_url(u)).any(|u| urls.contains(&u))
    };
    remotes.iter().filter(|r| named(r)).map(|r| r.name.clone()).collect()
}

/// The URL a `FETCH_HEAD` line ends in: `<oid>\t[not-for-merge]\t<what> of <url>`, or only the
/// URL when the remote's HEAD was fetched.
fn line_url(line: &str) -> Option<String> {
    let what = line.splitn(3, '\t').nth(2)?.trim();
    let url = what.rsplit_once(" of ").map_or(what, |(_, u)| u).trim();
    (!url.is_empty()).then(|| url.to_owned())
}

/// `url` as git writes it in `FETCH_HEAD`: no user name or password (`scheme://user@host/...`
/// and scp-like `user@host:path` alike), and no trailing `/` or `.git`.
fn bare_url(url: &str) -> String {
    let anonymous = anonymous(url);
    let trimmed = anonymous.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    trimmed.trim_end_matches('/').to_owned()
}

/// `url` without its `user[:password]@`, by git's rule: the text up to the first `@` goes when
/// the URL isn't a local path and the `@` sits in the host part (after `scheme://` and before
/// the first `/`, or before the `:` of an scp-like URL).
fn anonymous(url: &str) -> String {
    let Some(at) = url.find('@') else { return url.to_owned() };
    if is_local(url) {
        return url.to_owned();
    }
    let rest = &url[at + 1..];
    match url.find("://") {
        Some(s) if !url[s + 3..at].contains('/') => format!("{}{rest}", &url[..s + 3]),
        None if rest.contains(':') => rest.to_owned(),
        _ => url.to_owned(),
    }
}

/// Whether git reads `url` as a local path: no `:` before the first `/`, or a drive letter.
fn is_local(url: &str) -> bool {
    let b = url.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return true;
    }
    match (url.find(':'), url.find('/')) {
        (None, _) => true,
        (Some(c), Some(s)) => s < c,
        (Some(_), None) => false,
    }
}

fn modified(file: &Path) -> Option<i64> {
    let time = std::fs::metadata(file).ok()?.modified().ok()?;
    let secs = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::{LoadOptions, load};
    use crate::testkit::git_cli;
    use crate::testutil::{git, scratch_repo, with_origin, write};
    use std::time::{Duration, SystemTime};

    fn now() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
    }

    /// Writes `dir`/FETCH_HEAD with its modification time `secs` after the epoch.
    fn fetch_head_at(dir: &Path, secs: u64) {
        let file = dir.join("FETCH_HEAD");
        std::fs::write(&file, "").unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(secs)).unwrap();
    }

    #[test]
    fn missing_fetch_head_gives_none() {
        let dir = scratch_repo("fetch-time-none");
        write(&dir, "a.txt", "a\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "one"]);
        assert_eq!(load(&dir, &LoadOptions::all(10)).unwrap().fetched_at, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fetch_sets_the_time() {
        let (work, bare) = with_origin("fetch-time-fetch");
        git(&work, &["fetch", "-q", "origin"]);
        let at = load(&work, &LoadOptions::all(10)).unwrap().fetched_at.expect("FETCH_HEAD was written");
        assert!((now() - at).abs() < 60, "{at} is not about now");
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bare);
    }

    #[test]
    fn a_linked_worktree_takes_the_newer_file() {
        let main = scratch_repo("fetch-time-linked");
        write(&main, "a.txt", "a\n");
        git(&main, &["add", "."]);
        git(&main, &["commit", "-q", "-m", "one"]);
        let linked = main.with_file_name(format!("{}-wt", main.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&linked);
        git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
        let snap = load(&linked, &LoadOptions::all(10)).unwrap();
        let (own, common) = (snap.git_dir.clone(), snap.common.dir.clone());
        assert_ne!(own, common);
        fetch_head_at(&common, 2_000_000_000);
        fetch_head_at(&own, 1_000_000_000);
        assert_eq!(fetched_at(&own, &common), Some(2_000_000_000), "the common dir's is newer");
        fetch_head_at(&own, 2_100_000_000);
        assert_eq!(load(&linked, &LoadOptions::all(10)).unwrap().fetched_at, Some(2_100_000_000), "the worktree's is newer");
        let _ = std::fs::remove_dir_all(&linked);
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn urls_are_compared_as_git_writes_them() {
        let line = "abc\t\tbranch 'main' of https://github.com/acme/demo";
        assert_eq!(line_url(line).as_deref(), Some("https://github.com/acme/demo"));
        assert_eq!(line_url("abc\tnot-for-merge\t/srv/repos/a b").as_deref(), Some("/srv/repos/a b"), "a HEAD fetch is only its URL");
        assert_eq!(line_url("abc"), None);
        assert_eq!(bare_url("https://me:token@github.com/acme/demo.git/"), "https://github.com/acme/demo");
        assert_eq!(bare_url("git@github.com:acme/demo.git"), "github.com:acme/demo", "scp-like URLs lose their user too");
        assert_eq!(bare_url("ssh://git@host.example/path/demo.git"), "ssh://host.example/path/demo");
        assert_eq!(bare_url("github.com:acme/demo.git"), "github.com:acme/demo");
        assert_eq!(bare_url("/srv/me@home/demo"), "/srv/me@home/demo", "a local path keeps its @");
        assert_eq!(bare_url("C:/me@home/demo"), "C:/me@home/demo", "so does a drive path");
        assert_eq!(bare_url("https://host/a@b/demo"), "https://host/a@b/demo", "an @ in the path stays");
        assert_eq!(bare_url("/srv/repos/demo/"), "/srv/repos/demo");
    }

    #[test]
    fn a_fetch_names_the_remotes_it_covered() {
        let (work, bare) = with_origin("fetch-time-remotes");
        let other = bare.with_file_name(format!("{}-other", bare.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&other);
        git(&work, &["clone", "-q", "--bare", &bare.display().to_string(), &other.display().to_string()]);
        git(&work, &["remote", "add", "upstream", &other.display().to_string()]);
        git(&work, &["fetch", "-q", "upstream"]);
        assert_eq!(load(&work, &LoadOptions::all(10)).unwrap().fetched_remotes, ["upstream"]);
        git(&work, &["fetch", "-q", "--all"]);
        assert_eq!(load(&work, &LoadOptions::all(10)).unwrap().fetched_remotes, ["origin", "upstream"]);
        for d in [&work, &bare, &other] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Fetches over SSH through a stand-in `core.sshCommand` that runs the upload-pack command it
    /// is given in the bare repo's folder, so FETCH_HEAD shows the URL exactly as git writes it.
    #[test]
    fn ssh_remotes_are_named_as_git_writes_them() {
        let (work, bare) = with_origin("fetch-time-ssh");
        let slashed = |p: &Path| p.display().to_string().replace('\\', "/");
        let base = slashed(bare.parent().unwrap());
        let name = bare.file_name().unwrap().to_string_lossy().into_owned();
        let stem = name.strip_suffix(".git").unwrap_or(&name).to_owned();
        let script = work.with_file_name(format!("{}-fake-ssh.sh", work.file_name().unwrap().to_string_lossy()));
        // The last argument is the remote command; an ssh:// path starts with `/`, made relative.
        let body = format!("for a; do c=$a; done\ncd '{base}' || exit 1\nc=$(printf '%s' \"$c\" | sed \"s#'/#'#\")\neval \"$c\"\n");
        std::fs::write(&script, body).unwrap();
        let command = format!("sh '{}'", slashed(&script));
        git_cli(&work, &["config", "core.sshCommand", &command]);
        git_cli(&work, &["config", "ssh.variant", "simple"]);
        let cases = [
            ("scp", format!("git@example.com:{name}"), format!("example.com:{stem}")),
            ("ssh", format!("ssh://git@example.com/{name}"), format!("ssh://example.com/{stem}")),
        ];
        for (remote, url, written) in &cases {
            git_cli(&work, &["remote", "add", remote, url]);
            git_cli(&work, &["fetch", "-q", remote]);
            let head = std::fs::read_to_string(work.join(".git/FETCH_HEAD")).unwrap();
            let urls: Vec<String> = head.lines().filter_map(line_url).collect();
            assert!(!urls.is_empty() && urls.iter().all(|u| u == written), "{remote}: git wrote {urls:?}");
            assert_eq!(bare_url(url), *written);
            assert_eq!(load(&work, &LoadOptions::all(10)).unwrap().fetched_remotes, [*remote]);
        }
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_file(&script);
    }
}
