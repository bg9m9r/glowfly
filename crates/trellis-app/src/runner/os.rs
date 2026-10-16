//! Effects that hand something to the operating system: URLs, terminals, the file manager and
//! files opened in their default program. None of them touch the repository.

use crate::msg::{FilesMsg, Msg};
use std::path::Path;
use trellis_core::opens::{FileFacts, Os, opens_as_program};

fn failed(text: String) -> Option<Msg> {
    Some(Msg::Logged { ok: false, text })
}

/// The log line for a path that is gone, or None when it exists (a broken symlink counts).
fn missing(p: &Path) -> Option<Msg> {
    p.symlink_metadata().is_err().then(|| Msg::Logged { ok: false, text: format!("{} is not in the working tree", p.display()) })
}

/// Opens an `https://` link in the browser. Anything else (other schemes, file paths, links
/// with odd characters, an empty link) is refused, since links can come from a server.
pub(super) fn open_url(url: &str) -> Option<Msg> {
    if trellis_core::forge::https_host(url).is_none() {
        let shown = trellis_core::text::printable(url);
        return failed(if shown.is_empty() {
            "There is no link to open.".into()
        } else {
            format!("Refused to open {shown}: not an https link")
        });
    }
    open::that_detached(url).err().and_then(|e| failed(format!("Could not open {url}: {e}")))
}

pub(super) fn open_terminal(dir: &Path) -> Option<Msg> {
    crate::platform::open_terminal(dir).err().and_then(|e| failed(format!("Could not open a terminal: {e}")))
}

pub(super) fn open_folder(dir: &Path) -> Option<Msg> {
    crate::platform::open_in_file_manager(dir);
    None
}

/// Opens the working-tree file `rel` of `workdir` in its default program, without waiting for
/// it. Unless `confirmed`, a file the system would run (see [`runs`]) comes back as
/// `OpenNeedsConfirm` instead.
pub(super) fn open_path(workdir: &Path, rel: &str, confirmed: bool) -> Option<Msg> {
    let p = workdir.join(rel);
    if let Some(m) = missing(&p) {
        return Some(m);
    }
    if !confirmed && let Some(kind) = runs(&p) {
        return Some(Msg::Files(FilesMsg::OpenNeedsConfirm { workdir: workdir.to_owned(), path: rel.to_owned(), kind }));
    }
    open::that_detached(&p).err().and_then(|e| failed(format!("Could not open {}: {e}", p.display())))
}

/// What `p` is when opening it would run it: by its name, by what a symbolic link there points
/// to, and by its executable bit and type (following the link), with this system's `PATHEXT`.
fn runs(p: &Path) -> Option<&'static str> {
    let facts = std::fs::metadata(p).ok().map(|m| FileFacts::of(&m));
    let pathext = std::env::var("PATHEXT").ok();
    let target = std::fs::read_link(p).ok();
    [Some(p), target.as_deref()].into_iter().flatten().find_map(|q| opens_as_program(q, facts, Os::current(), pathext.as_deref()))
}

/// Shows `p` selected in the file manager (its folder, where selecting isn't supported).
pub(super) fn reveal(p: &Path) -> Option<Msg> {
    if let Some(m) = missing(p) {
        return Some(m);
    }
    crate::platform::reveal_in_file_manager(p).err().and_then(|e| failed(format!("Could not show {}: {e}", p.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logged(m: Option<Msg>) -> (bool, String) {
        match m {
            Some(Msg::Logged { ok, text }) => (ok, text),
            _ => panic!("expected a log line"),
        }
    }

    #[test]
    fn only_https_links_are_opened() {
        for url in ["", "file:///C:/Windows/System32/calc.exe", "javascript:alert(1)", r"C:\Windows\notepad.exe", "https://x.example/a b"] {
            let (ok, text) = logged(open_url(url));
            assert!(!ok, "{url}");
            assert!(text.contains("no link") || text.contains("not an https link"), "{text}");
        }
    }

    #[test]
    fn missing_paths_are_logged_without_launching_anything() {
        let dir = std::env::temp_dir().join(format!("trellis-no-such-file-{}", std::process::id()));
        let p = dir.join("gone.txt");
        for m in [open_path(&dir, "gone.txt", false), open_path(&dir, "gone.txt", true), reveal(&p)] {
            let (ok, text) = logged(m);
            assert!(!ok);
            assert!(text.contains("not in the working tree"), "{text}");
        }
    }

    #[test]
    fn files_the_system_would_run_are_found_on_disk() {
        let dir = std::env::temp_dir().join(format!("trellis-runs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "text").unwrap();
        assert_eq!(runs(&dir.join("notes.txt")), None);
        let program = if cfg!(windows) { "run.bat" } else { "run" };
        std::fs::write(dir.join(program), "@exit /b 0").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(runs(&dir.join(program)), None, "not executable yet");
            std::fs::set_permissions(dir.join(program), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(runs(&dir.join(program)).is_some());
        // A link with a harmless name that points at it counts too.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join(program), dir.join("readme.txt")).unwrap();
            assert!(runs(&dir.join("readme.txt")).is_some());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
