//! Effects that hand something to the operating system: URLs, terminals, the file manager and
//! files opened in their default program. None of them touch the repository.

use crate::msg::Msg;
use std::path::Path;

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
    crate::platform::open_terminal(dir);
    None
}

pub(super) fn open_folder(dir: &Path) -> Option<Msg> {
    crate::platform::open_in_file_manager(dir);
    None
}

/// Opens `p` in its default program, without waiting for it.
pub(super) fn open_path(p: &Path) -> Option<Msg> {
    if let Some(m) = missing(p) {
        return Some(m);
    }
    open::that_detached(p).err().and_then(|e| failed(format!("Could not open {}: {e}", p.display())))
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
        let p = std::env::temp_dir().join(format!("trellis-no-such-file-{}", std::process::id())).join("gone.txt");
        for m in [open_path(&p), reveal(&p)] {
            let (ok, text) = logged(m);
            assert!(!ok);
            assert!(text.contains("not in the working tree"), "{text}");
        }
    }
}
