//! Whether "open in the default program" would run a working-tree file rather than show it.
//!
//! Files that come from a repository carry no Mark of the Web (Windows) or quarantine flag
//! (macOS), so the system runs a program, script, shortcut or installer from one without any
//! warning of its own. Trellis asks first for those. The rules are per operating system, and
//! take the system as a parameter so every system's table is tested everywhere.

use std::path::Path;

/// The operating system whose default-program rules apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    /// Linux and the other freedesktop systems.
    Linux,
}

impl Os {
    pub const fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// What the file system says about the file, when it was looked at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub is_dir: bool,
    /// Any executable bit is set (Unix).
    pub executable: bool,
}

impl FileFacts {
    pub fn of(meta: &std::fs::Metadata) -> FileFacts {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            meta.is_file() && meta.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        FileFacts { is_dir: meta.is_dir(), executable }
    }
}

/// Windows' own list when `PATHEXT` isn't set.
pub const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC";

/// Extensions Windows runs, opens as a shortcut or installs, beyond `PATHEXT`.
const WINDOWS_RUNS: &[(&str, &str)] = &[
    ("exe", "a program"),
    ("com", "a program"),
    ("scr", "a program"),
    ("pif", "a program"),
    ("cpl", "a program"),
    ("msc", "a program"),
    ("bat", "a script"),
    ("cmd", "a script"),
    ("ps1", "a script"),
    ("vbs", "a script"),
    ("vbe", "a script"),
    ("js", "a script"),
    ("jse", "a script"),
    ("wsf", "a script"),
    ("wsh", "a script"),
    ("hta", "a program"),
    ("jar", "a program"),
    ("lnk", "a shortcut"),
    ("url", "a shortcut"),
    ("appref-ms", "a shortcut"),
    ("application", "an installer"),
    ("msi", "an installer"),
    ("msp", "an installer"),
    ("reg", "a registry file"),
];

const MAC_RUNS: &[(&str, &str)] = &[
    ("app", "a program"),
    ("command", "a script"),
    ("tool", "a script"),
    ("terminal", "a Terminal file"),
    ("jar", "a program"),
    ("pkg", "an installer"),
    ("mpkg", "an installer"),
];

const LINUX_RUNS: &[(&str, &str)] = &[("desktop", "a launcher"), ("jar", "a program")];

/// What `path` is when opening it in its default program on `os` would run it ("a program",
/// "a script", "a shortcut", ...), or `None` when it is only opened. `facts` comes from the
/// file system when it was looked at (without it, only the name counts); `pathext` is Windows'
/// `PATHEXT` (Windows' default list without it).
pub fn opens_as_program(path: &Path, facts: Option<FileFacts>, os: Os, pathext: Option<&str>) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy();
    // Windows ignores trailing dots and spaces: `run.bat.` is `run.bat`.
    let name = if os == Os::Windows { name.trim_end_matches(['.', ' ']) } else { &name };
    let ext =
        name.rsplit_once('.').map(|(stem, ext)| (stem, ext.to_ascii_lowercase())).filter(|(stem, _)| !stem.is_empty()).map(|(_, e)| e);
    let listed = |table: &[(&str, &'static str)]| ext.as_deref().and_then(|e| table.iter().find(|(x, _)| *x == e).map(|(_, kind)| *kind));
    let facts = facts.unwrap_or_default();
    match os {
        Os::Windows => listed(WINDOWS_RUNS).or_else(|| {
            let ext = ext.as_deref()?;
            pathext
                .unwrap_or(DEFAULT_PATHEXT)
                .split(';')
                .any(|p| p.trim().trim_start_matches('.').eq_ignore_ascii_case(ext))
                .then_some("a program")
        }),
        // An executable file without an extension opens in Terminal, which runs it.
        Os::MacOs => listed(MAC_RUNS).or_else(|| (facts.executable && !facts.is_dir && ext.is_none()).then_some("an executable file")),
        Os::Linux => listed(LINUX_RUNS).or_else(|| (facts.executable && !facts.is_dir).then_some("an executable file")),
    }
}

#[cfg(test)]
mod tests;
