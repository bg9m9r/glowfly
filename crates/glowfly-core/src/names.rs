//! One rule set for the names Glowfly creates or compares: new files and folders, renames,
//! branch and tag names, clone and worktree folders, exported patches and SSH keys.
//!
//! - [`win_name_problem`]: what Windows refuses in one name (device names, some characters, a
//!   trailing dot or space). Such a name works on Linux and macOS but won't check out on
//!   Windows, so it is an error on Windows and a warning elsewhere ([`blocks_here`]).
//! - [`fold`]: the key under which a case- and normalization-insensitive file system (NTFS,
//!   APFS and HFS+ by default, FAT, exFAT, an ext4 casefold folder) sees two names as one.
//! - [`is_git_dir_name`]: whether a name reaches `.git` on some file system. A security check,
//!   so it overmatches on every OS rather than asking which file system it is on.

use icu_normalizer::ComposingNormalizerBorrowed;

/// Why Windows can't use one file or folder name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WinNameProblem {
    /// Reserved for a device whatever the extension (`aux.c` opens the AUX device). Holds the
    /// device name as typed.
    Device(String),
    /// `<>:"|?*` or a control character.
    Char(char),
    /// Windows drops a dot or space at the end, so the name would turn into another.
    Trailing,
}

impl WinNameProblem {
    /// Why, as a sentence fragment: "Windows reserves AUX for a device".
    pub fn reason(&self) -> String {
        match self {
            WinNameProblem::Device(d) => format!("Windows reserves {d} for a device"),
            WinNameProblem::Char(c) if c.is_control() => "Windows doesn't allow control characters in names".to_owned(),
            WinNameProblem::Char(c) => format!("Windows doesn't allow {c} in names"),
            WinNameProblem::Trailing => "Windows drops a dot or space at the end of a name".to_owned(),
        }
    }

    /// The warning shown where the name still works: "<reason>, so <shown> won't check out on
    /// Windows."
    pub fn warning(&self, shown: &str) -> String {
        format!("{}, so {shown} won't check out on Windows.", self.reason())
    }
}

/// Whether a [`WinNameProblem`] is refused here (Windows) rather than shown as a warning.
pub const fn blocks_here() -> bool {
    cfg!(windows)
}

const DEVICES: &[&str] = &["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];
const BAD_CHARS: &str = "<>:\"|?*";

/// What Windows refuses in the single name `name` (no separators): the device names CON, PRN,
/// AUX, NUL, CONIN$, CONOUT$, COM0-9, LPT0-9 and COM¹²³/LPT¹²³ in any case and with any
/// extension; the characters `<>:"|?*` and control characters; a trailing dot or space.
pub fn win_name_problem(name: &str) -> Option<WinNameProblem> {
    let stem = device_stem(name);
    if is_device(stem) {
        return Some(WinNameProblem::Device(stem.to_owned()));
    }
    if let Some(c) = name.chars().find(|c| c.is_control() || BAD_CHARS.contains(*c)) {
        return Some(WinNameProblem::Char(c));
    }
    name.ends_with(['.', ' ']).then_some(WinNameProblem::Trailing)
}

/// The part of `name` Windows matches against device names: up to the first '.' (or ':'),
/// without trailing spaces ("con .txt" is CON too).
fn device_stem(name: &str) -> &str {
    name.split(['.', ':']).next().unwrap_or(name).trim_end_matches(' ')
}

fn is_device(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    if DEVICES.contains(&upper.as_str()) {
        return true;
    }
    let Some(number) = upper.strip_prefix("COM").or_else(|| upper.strip_prefix("LPT")) else { return false };
    let mut chars = number.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_digit() || matches!(c, '¹' | '²' | '³'))
}

/// `name` changed just enough to pass [`win_name_problem`]: refused characters become '-', a
/// trailing dot or space goes, and a device name gets `suffix` after its stem ("aux" with
/// "-wt" is "aux-wt", "con.d" is "con-wt.d"). For names Glowfly makes up itself, never for
/// what the user typed. Can return "" (for "..."); callers have their own fallback.
pub fn portable(name: &str, suffix: &str) -> String {
    let mapped: String = name.chars().map(|c| if c.is_control() || BAD_CHARS.contains(c) { '-' } else { c }).collect();
    let mut out = mapped.trim_end_matches(['.', ' ']).to_owned();
    let stem = device_stem(&out);
    if !stem.is_empty() && is_device(stem) {
        let at = stem.len();
        out = format!("{}{suffix}{}", &out[..at], out[at..].trim_start_matches(' '));
    }
    out
}

/// The first [`win_name_problem`] in the '/'-separated `path`, with the name it is in.
pub fn path_problem(path: &str) -> Option<(&str, WinNameProblem)> {
    path.split('/').find_map(|part| win_name_problem(part).map(|p| (part, p)))
}

/// `s` in Unicode Normalization Form C (composed: "e\u{301}" becomes "é").
pub fn nfc(s: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc().normalize(s).into_owned()
}

/// The key under which a case- and normalization-insensitive file system sees `s`: composed,
/// lowercased, composed again (lowercasing can decompose).
pub fn fold(s: &str) -> String {
    nfc(&nfc(s).to_lowercase())
}

/// Whether two names are one to a case- and normalization-insensitive file system.
pub fn same_folded(a: &str, b: &str) -> bool {
    a == b || fold(a) == fold(b)
}

/// Code points HFS+ ignores in names, so ".g\u{200C}it" is ".git" there: zero-width joiners
/// and marks (U+200C-200F), bidirectional embeddings (U+202A-202E), deprecated format
/// characters (U+206A-206F) and the byte-order mark (U+FEFF). The same set git refuses with
/// core.protectHFS.
pub fn is_hfs_ignorable(c: char) -> bool {
    matches!(c, '\u{200C}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{206A}'..='\u{206F}' | '\u{FEFF}')
}

/// Whether the single name `part` would reach `.git` on some supported file system: HFS+
/// ignorable code points dropped, composed and case-folded, then what NTFS ignores (a trailing
/// dot or space, an alternate data stream after ':'), or NTFS's short name GIT~1. Checked the
/// same way on every OS, because a repository made on one is checked out on the others.
pub fn is_git_dir_name(part: &str) -> bool {
    let stripped: String = part.chars().filter(|c| !is_hfs_ignorable(*c)).collect();
    let folded = fold(&stripped);
    let name = folded.split(':').next().unwrap_or(&folded).trim_end_matches(['.', ' ']);
    name == ".git" || name == "git~1"
}

/// How a new ref name collides with existing refs of its kind (branches, or tags) when case
/// and Unicode form are ignored, as they are for loose ref files on Windows and macOS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefClash {
    /// That ref, spelled like this.
    Exists(String),
    /// That ref is a folder of the new name, or the other way round.
    Clashes(String),
    /// The new name's folder `typed` already exists as `existing`: on those systems the ref
    /// would go into that folder and be listed under its spelling.
    FolderCase { typed: String, existing: String },
}

/// The first way `name` collides with one of `existing`.
pub fn ref_clash<'a>(name: &str, existing: impl IntoIterator<Item = &'a str>) -> Option<RefClash> {
    let key = fold(name);
    let folders = |n: &'a str| n.match_indices('/').map(move |(i, _)| &n[..i]);
    let mut folder_case = None;
    for other in existing {
        let o = fold(other);
        if o == key {
            return Some(RefClash::Exists(other.to_owned()));
        }
        if o.starts_with(&format!("{key}/")) || key.starts_with(&format!("{o}/")) {
            return Some(RefClash::Clashes(other.to_owned()));
        }
        if folder_case.is_none()
            && let Some((typed, existing)) =
                name.match_indices('/').map(|(i, _)| &name[..i]).zip(folders(other)).find(|(t, e)| t != e && same_folded(t, e))
        {
            folder_case = Some(RefClash::FolderCase { typed: typed.to_owned(), existing: existing.to_owned() });
        }
    }
    folder_case
}

/// The first part of the ref name `name` that Windows can't use as a file or folder name
/// (loose refs are files under .git/refs).
pub fn ref_name_problem(name: &str) -> Option<(&str, WinNameProblem)> {
    path_problem(name)
}

#[cfg(test)]
mod tests;
