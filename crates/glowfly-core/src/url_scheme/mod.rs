//! Registering Glowfly as the program for `glowfly://` links, for this user only.
//!
//! - Windows: the per-user class key `HKEY_CURRENT_USER\Software\Classes\glowfly`, whose
//!   `shell\open\command` runs `"<glowfly.exe>" "%1"`.
//! - Linux and the other XDG desktops: a hidden desktop entry, `glowfly-links.desktop` in the
//!   data folder's `applications` (`$XDG_DATA_HOME`, else `~/.local/share`), declaring
//!   `x-scheme-handler/glowfly`, made the default with `xdg-mime default`. The Linux package
//!   installs the same file for [`PACKAGED_EXE`] in `/usr/share/applications`, which counts too.
//! - macOS: Glowfly.app declares the scheme in its Info.plist and receives links as Apple
//!   events (glowfly-app's platform/macos_events.rs); LaunchServices registers the bundle when it
//!   is installed, so there is nothing to register here, and [`status`] says so.
//!
//! - Inside Flatpak: the exported `io.github.bg9m9r.glowfly.links.desktop` declares the scheme,
//!   so nothing is registered ([`FLATPAK_NOTE`]; glowfly-app's runner/deep_link.rs).
//!
//! The decisions (what a registry value or `xdg-mime` answer means, how the command line is
//! quoted) are pure functions, tested on every OS.

#[cfg(all(unix, not(target_os = "macos")))]
mod xdg;
#[cfg(all(unix, not(target_os = "macos")))]
use xdg as imp;
// Not `windows`: that would shadow the crate of the same name.
#[cfg(windows)]
#[path = "windows.rs"]
mod win;
#[cfg(windows)]
use win as imp;

#[cfg(test)]
mod tests;

use std::path::Path;

/// The scheme.
pub const SCHEME: &str = "glowfly";
/// The desktop entry's file name on XDG desktops.
pub const DESKTOP_FILE: &str = "glowfly-links.desktop";
/// Where the Linux package installs Glowfly; its packaged `glowfly-links.desktop` (in
/// `/usr/share/applications`) is `desktop_entry` for this path.
pub const PACKAGED_EXE: &str = "/usr/bin/glowfly";
/// Why macOS has nothing to register: Glowfly.app's Info.plist declares the scheme.
pub const MACOS_NOTE: &str = "On macOS, links will open Glowfly once it is installed as an application.";
/// Why a Flatpak copy has nothing to register: its exported desktop file declares the scheme.
pub const FLATPAK_NOTE: &str = "Flatpak registers glowfly:// links for Glowfly when it is installed.";
/// The desktop's handler query lacks its program.
pub const NO_XDG_MIME: &str = "xdg-mime wasn't found; install xdg-utils to register links.";

/// Which program opens `glowfly://` links for this user.
#[derive(Clone, Debug, PartialEq)]
pub enum SchemeStatus {
    /// This copy of Glowfly.
    Registered,
    /// Another program, or another copy of Glowfly: its command line or desktop file.
    OtherProgram(String),
    NotRegistered,
    /// This system can't be registered with yet; the reason.
    Unsupported(&'static str),
}

/// The registry command for `exe`: `"<exe>" "%1"`, in Windows' own spelling (no `\\?\`).
pub fn windows_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", crate::native_path::native(exe).display())
}

/// The desktop entry that hands `glowfly://` links to `exe`. Refuses a path that isn't UTF-8 or
/// holds control characters, which a desktop file can't carry.
pub fn desktop_entry(exe: &Path) -> Result<String, String> {
    let exe = exe.to_str().ok_or_else(|| format!("{} isn't valid UTF-8, which a desktop file needs", exe.display()))?;
    if exe.chars().any(char::is_control) {
        return Err("Glowfly's path contains control characters, which a desktop file can't hold".to_owned());
    }
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Glowfly\nComment=Opens glowfly:// links in Glowfly\nExec={} %u\n\
         Terminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/{SCHEME};\n",
        desktop_exec_arg(exe)
    ))
}

/// One argument of a desktop entry's `Exec` line: double-quoted when it holds a reserved
/// character, with `"`, `` ` ``, `$` and `\` escaped inside the quotes; `%` doubled (it starts a
/// field code); then the backslashes escaped again, as every desktop-file string value is.
pub(crate) fn desktop_exec_arg(s: &str) -> String {
    const RESERVED: &[char] = &[' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`'];
    let quoted = if s.contains(RESERVED) {
        let mut q = String::from('"');
        for c in s.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                q.push('\\');
            }
            q.push(c);
        }
        q.push('"');
        q
    } else {
        s.to_owned()
    };
    quoted.replace('%', "%%").replace('\\', "\\\\")
}

/// The `Exec=` value of a desktop file's text.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn exec_line(text: &str) -> Option<&str> {
    text.lines().find_map(|l| l.trim_start().strip_prefix("Exec="))
}

/// What `xdg-mime query default x-scheme-handler/glowfly` printed, read with Glowfly's own
/// desktop file (None when it isn't there), means for `exe`.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
pub(crate) fn decide_xdg(query_default: &str, our_file: Option<&str>, exe: &Path) -> SchemeStatus {
    let default = query_default.trim();
    if default.is_empty() {
        return SchemeStatus::NotRegistered;
    }
    if default != DESKTOP_FILE {
        return SchemeStatus::OtherProgram(default.to_owned());
    }
    // A default naming a file that is gone is ignored by desktops.
    let Some(file) = our_file else { return SchemeStatus::NotRegistered };
    let want = desktop_entry(exe).ok();
    match (exec_line(file), want.as_deref().and_then(exec_line)) {
        (Some(have), Some(want)) if have == want => SchemeStatus::Registered,
        (Some(have), _) => SchemeStatus::OtherProgram(have.to_owned()),
        (None, _) => SchemeStatus::OtherProgram(DESKTOP_FILE.to_owned()),
    }
}

/// What the registry's `shell\open\command` value (None when missing) means for `exe`.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn decide_windows(command: Option<&str>, exe: &Path) -> SchemeStatus {
    match command.map(str::trim) {
        None | Some("") => SchemeStatus::NotRegistered,
        // Windows paths ignore case.
        Some(c) if c.eq_ignore_ascii_case(&windows_command(exe)) => SchemeStatus::Registered,
        Some(c) => SchemeStatus::OtherProgram(c.to_owned()),
    }
}

/// Which program opens links now.
pub fn status(exe: &Path) -> Result<SchemeStatus, String> {
    imp::status(exe)
}

/// Makes `exe` the program for links, then reads the status again.
pub fn register(exe: &Path) -> Result<SchemeStatus, String> {
    imp::register(exe)
}

/// Removes Glowfly's registration, then reads the status again (`exe` is only compared with).
pub fn unregister(exe: &Path) -> Result<SchemeStatus, String> {
    imp::unregister(exe)
}

/// Systems without a per-user registration Glowfly can write yet (macOS).
#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
mod imp {
    use super::{MACOS_NOTE, SchemeStatus};
    use std::path::Path;

    pub(super) fn status(_exe: &Path) -> Result<SchemeStatus, String> {
        Ok(SchemeStatus::Unsupported(MACOS_NOTE))
    }

    pub(super) fn register(_exe: &Path) -> Result<SchemeStatus, String> {
        Ok(SchemeStatus::Unsupported(MACOS_NOTE))
    }

    pub(super) fn unregister(_exe: &Path) -> Result<SchemeStatus, String> {
        Ok(SchemeStatus::Unsupported(MACOS_NOTE))
    }
}
