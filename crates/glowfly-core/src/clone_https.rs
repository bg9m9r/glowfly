//! Cloning over https from a host the user connected a hosting account for: the `-c` options
//! that send that host's SSH URLs (a superproject's `.gitmodules` often names its submodules
//! as `git@host:owner/repo.git`) over https for one command, and the https URL of an SSH clone
//! URL, for the "Clone over HTTPS" offer after an SSH clone fails. Pure: no I/O.

use crate::forge::split_url;

/// Whether `host` is a plain host name (letters, digits, `-`, `.`): nothing that could end a
/// config key or URL early.
fn plain_host(host: &str) -> bool {
    !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
}

/// `-c` options for one git command that fetch `host`'s SSH URLs (scp-like `git@host:` and
/// `ssh://git@host/`) from `https://host/` instead. git hands `-c` settings on to the commands it
/// runs itself, so a clone's submodules get them too. Empty for a host that isn't plain.
pub fn https_rewrites(host: &str) -> Vec<String> {
    let host = host.to_ascii_lowercase();
    if !plain_host(&host) {
        return Vec::new();
    }
    let key = format!("url.https://{host}/.insteadOf");
    vec!["-c".into(), format!("{key}=git@{host}:"), "-c".into(), format!("{key}=ssh://git@{host}/")]
}

/// The host and https clone URL of an SSH clone URL (`git@github.com:ada/app.git` →
/// `https://github.com/ada/app.git`). None for any other URL, a host that isn't plain, or a
/// path with characters a URL would need to escape.
pub fn https_for_ssh(url: &str) -> Option<(String, String)> {
    let (host, segments, ssh) = split_url(url)?;
    let safe = |s: &String| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'));
    if !ssh || !plain_host(&host) || segments.is_empty() || !segments.iter().all(safe) {
        return None;
    }
    let url = format!("https://{host}/{}.git", segments.join("/"));
    Some((host, url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_both_ssh_spellings_of_the_host() {
        assert_eq!(
            https_rewrites("GitHub.com"),
            ["-c", "url.https://github.com/.insteadOf=git@github.com:", "-c", "url.https://github.com/.insteadOf=ssh://git@github.com/"]
        );
        assert!(https_rewrites("evil.com/.x=y").is_empty());
        assert!(https_rewrites("").is_empty());
    }

    #[test]
    fn https_url_of_an_ssh_url() {
        let gh = Some(("github.com".to_owned(), "https://github.com/bg9m9r/majik.git".to_owned()));
        assert_eq!(https_for_ssh("git@github.com:bg9m9r/majik.git"), gh);
        assert_eq!(https_for_ssh("ssh://git@github.com/bg9m9r/majik"), gh);
        assert_eq!(https_for_ssh("git@ssh.github.com:bg9m9r/majik.git"), gh, "the port-443 alias is github.com");
        assert_eq!(https_for_ssh("git@gitlab.com:g/sub/app.git").unwrap().1, "https://gitlab.com/g/sub/app.git");
        assert_eq!(https_for_ssh("https://github.com/bg9m9r/majik.git"), None, "already https");
        assert_eq!(https_for_ssh("C:/repos/app"), None);
        assert_eq!(https_for_ssh("git@github.com:a/b c.git"), None);
        assert_eq!(https_for_ssh("git@github.com:"), None);
    }

    /// The mechanism `https_rewrites` relies on: a `-c url.<base>.insteadOf` given to `git clone
    /// --recurse-submodules` reaches the submodule clones, so a submodule named by an SSH URL is
    /// fetched from the rewritten base. Here the base is a folder, standing in for https.
    #[test]
    fn a_clones_url_rewrite_reaches_its_submodules() {
        use crate::testutil::{git, scratch_repo, write};
        let lib = scratch_repo("clone-https-lib");
        write(&lib, "lib.txt", "lib");
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "lib"]);
        let src = scratch_repo("clone-https-src");
        let file_base = format!("file:///{}/", lib.parent().unwrap().display().to_string().replace('\\', "/").trim_start_matches('/'));
        let lib_name = lib.file_name().unwrap().to_string_lossy().into_owned();
        git(&src, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &format!("{file_base}{lib_name}"), "lib"]);
        // The superproject names it the way a hosted one would: by an SSH URL.
        git(&src, &["config", "-f", ".gitmodules", "submodule.lib.url", &format!("git@fake.invalid:{lib_name}")]);
        git(&src, &["commit", "-q", "-am", "subs"]);
        let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();
        let rewrite = format!("url.{file_base}.insteadOf=git@fake.invalid:");
        let src_url = src.display().to_string();
        let cmd = ["-c", &rewrite, "-c", "protocol.file.allow=always", "clone", "--recurse-submodules", "--", &src_url, "copy"];
        let (ok, out) = crate::gitcli::run_with_input(&parent, &crate::gitcli::args(&cmd), "", &[]);
        assert!(ok, "{out}");
        assert!(parent.join("copy/lib/lib.txt").is_file(), "{out}");
        for d in [&parent, &src, &lib] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
