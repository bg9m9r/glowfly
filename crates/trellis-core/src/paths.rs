//! One folder, many spellings: a Windows short 8.3 name (`C:\Users\RUNNER~1`) and its long
//! form, a link or junction and its target, macOS's `/var` and `/private/var`. libgit2 and git
//! report the resolved form, while a typed path, `%TEMP%` or a recent-list entry may use any
//! other, so paths that came from outside are resolved here before they are compared with the
//! ones git reports.

use std::path::{Component, Path, PathBuf};

/// `path` as it really is on disk: links, junctions and short 8.3 names followed, in its plain
/// form (no `\\?\` prefix on Windows). A path that doesn't exist yet (a clone's destination,
/// say) has its nearest existing parent resolved and the rest kept as typed. `path` itself when
/// nothing of it can be resolved.
pub fn resolve(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut base = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(base) {
            let mut out = plain(&real);
            out.extend(rest.iter().rev());
            return out;
        }
        match (base.file_name(), base.parent()) {
            (Some(name), Some(parent)) if !parent.as_os_str().is_empty() => {
                rest.push(name.to_owned());
                base = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The system's temporary folder, resolved (`std::env::temp_dir` may give its short 8.3 name on
/// Windows, or `/var/...` for `/private/var/...` on macOS).
pub fn temp_dir() -> PathBuf {
    resolve(&std::env::temp_dir())
}

/// `path` without the verbatim `\\?\` prefix `canonicalize` adds on Windows, when it has a plain
/// equivalent (a drive or a UNC share); unchanged otherwise.
pub fn plain(path: &Path) -> PathBuf {
    use std::path::Prefix;
    let mut parts = path.components();
    let Some(Component::Prefix(p)) = parts.next() else { return path.to_path_buf() };
    let head = match p.kind() {
        Prefix::VerbatimDisk(d) => format!("{}:", d as char),
        Prefix::VerbatimUNC(server, share) => format!(r"\\{}\{}", server.to_string_lossy(), share.to_string_lossy()),
        _ => return path.to_path_buf(),
    };
    let mut out = PathBuf::from(head);
    out.extend(parts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = temp_dir().join(format!("trellis-paths-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        resolve(&d)
    }

    #[test]
    fn resolve_follows_a_link_and_keeps_a_missing_tail() {
        let root = dir("link");
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("alias");
        #[cfg(windows)]
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&target, &link).is_ok();
        assert!(made, "made the link");
        assert_eq!(resolve(&link), target);
        assert_eq!(resolve(&link.join("not-yet").join("deeper")), target.join("not-yet").join("deeper"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_leaves_what_it_cannot_find() {
        let p = Path::new("no-such-relative-folder-for-trellis");
        assert_eq!(resolve(p), p);
    }

    /// The short 8.3 alias of a folder, when the volume makes them (CI runners do; many
    /// developer machines have them turned off).
    #[cfg(windows)]
    fn short_name(p: &Path) -> Option<PathBuf> {
        let out =
            std::process::Command::new("cmd").arg("/c").arg(format!("for %I in (\"{}\") do @echo %~sI", p.display())).output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let short = PathBuf::from(s);
        (short != p && short.exists()).then_some(short)
    }

    #[cfg(windows)]
    #[test]
    fn resolve_expands_short_names() {
        let root = dir("short");
        let long = root.join("a folder with a long name");
        std::fs::create_dir_all(&long).unwrap();
        match short_name(&long) {
            Some(short) => {
                assert_eq!(resolve(&short), long);
                assert_eq!(resolve(&short.join("gone")), long.join("gone"));
            }
            None => eprintln!("note: this volume makes no 8.3 names; nothing to expand"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn plain_drops_the_verbatim_prefix() {
        assert_eq!(plain(Path::new(r"\\?\C:\work\repo")), Path::new(r"C:\work\repo"));
        assert_eq!(plain(Path::new(r"\\?\UNC\server\share\repo")), Path::new(r"\\server\share\repo"));
        assert_eq!(plain(Path::new(r"C:\work")), Path::new(r"C:\work"));
    }
}
