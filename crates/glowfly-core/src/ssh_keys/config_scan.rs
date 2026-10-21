//! Reading `~/.ssh/config` and the files it Includes for their `IdentityFile` entries. Touches
//! only the file system (no programs); call it off the UI thread.
//!
//! Include takes several names, each relative to the .ssh folder unless it is a full path or
//! starts with `~`; `*` and `?` may appear in the last part of a name, and the matches are read
//! in lexical order. A name that matches nothing is skipped, as ssh does. Nesting stops at 16
//! levels (OpenSSH's limit), and a file already being read is skipped, which breaks cycles.

use super::config::{Directive, parse_config};
use super::config_expand::{ExpandCx, Unresolved, absolute_for, expand_identity};
use crate::paths::{expand_home, resolve, same_spelling};
use std::path::{Path, PathBuf};

/// How deep Includes nest (OpenSSH's READCONF_MAX_DEPTH).
const MAX_DEPTH: usize = 16;

/// One `IdentityFile` entry found in the config files.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedIdentity {
    /// The value as written.
    pub raw: String,
    /// The config file that holds it.
    pub file: PathBuf,
    /// Its 1-based line number.
    pub line: usize,
    /// The `Host` or `Match` line it is under ("" before the first one).
    pub section: String,
    /// The file it names, or why that can't be known here.
    pub path: Result<PathBuf, Unresolved>,
}

/// What reading the config files found.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ConfigScan {
    pub entries: Vec<ScannedIdentity>,
    /// Why `~/.ssh/config` itself couldn't be read (it is not an error for it to be missing).
    pub error: Option<String>,
}

/// The IdentityFile entries of `ssh_dir/config` and the files it Includes, in the order ssh
/// reads them.
pub fn scan_config(ssh_dir: &Path, cx: &ExpandCx) -> ConfigScan {
    let file = ssh_dir.join("config");
    let text = match std::fs::read(&file) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ConfigScan::default(),
        Err(e) => return ConfigScan { entries: Vec::new(), error: Some(e.to_string()) },
    };
    let mut scan = Scan { ssh_dir, cx, stack: vec![resolve(&file)], entries: Vec::new() };
    scan.read(&file, &text, "", 0);
    ConfigScan { entries: scan.entries, error: None }
}

struct Scan<'a> {
    ssh_dir: &'a Path,
    cx: &'a ExpandCx<'a>,
    /// The files being read, resolved, outermost first.
    stack: Vec<PathBuf>,
    entries: Vec<ScannedIdentity>,
}

impl Scan<'_> {
    /// Reads one file's `text` at `depth`, its leading entries under `section`.
    fn read(&mut self, file: &Path, text: &str, section: &str, depth: usize) {
        let mut section = section.to_owned();
        for d in parse_config(text) {
            match d {
                Directive::Section { text, .. } => section = text,
                Directive::Identity { line, raw } => {
                    let path = expand_identity(&raw, self.cx);
                    self.entries.push(ScannedIdentity { raw, file: file.to_owned(), line, section: section.clone(), path });
                }
                Directive::Include { patterns, .. } => {
                    if depth >= MAX_DEPTH {
                        continue;
                    }
                    for pattern in &patterns {
                        for included in self.matches(pattern) {
                            self.include(&included, &section, depth + 1);
                        }
                    }
                }
            }
        }
    }

    /// Reads an included file, unless it is already being read or can't be read.
    fn include(&mut self, file: &Path, section: &str, depth: usize) {
        let real = resolve(file);
        if self.stack.iter().any(|f| same_spelling(f, &real)) {
            return;
        }
        let Ok(bytes) = std::fs::read(file) else { return };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        self.stack.push(real);
        self.read(file, &text, section, depth);
        self.stack.pop();
    }

    /// The files an Include name stands for, sorted by name when it has wildcards.
    fn matches(&self, pattern: &str) -> Vec<PathBuf> {
        let path = if pattern.starts_with('~') {
            expand_home(pattern, Some(self.cx.home))
        } else {
            match absolute_for(pattern, self.cx.windows) {
                Ok(s) => PathBuf::from(s),
                Err(Unresolved::Relative) => self.ssh_dir.join(pattern),
                Err(_) => return Vec::new(),
            }
        };
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else { return vec![path] };
        if !has_wildcard(&name) {
            return vec![path];
        }
        let Some(dir) = path.parent() else { return Vec::new() };
        // Wildcards in folder names aren't followed.
        if dir.to_str().is_none_or(has_wildcard) {
            return Vec::new();
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut names: Vec<String> =
            entries.flatten().filter_map(|e| e.file_name().to_str().map(str::to_owned)).filter(|n| name_matches(&name, n)).collect();
        names.sort();
        names.into_iter().map(|n| dir.join(n)).filter(|p| p.is_file()).collect()
    }
}

fn has_wildcard(s: &str) -> bool {
    s.contains(['*', '?'])
}

/// Whether a file name matches a glob(7) pattern of `*` and `?`. A leading `.` must be matched
/// by a `.` in the pattern.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    if name.starts_with('.') && !pattern.starts_with('.') {
        return false;
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    // The classic greedy match with one backtrack point for the last `*`.
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ni));
            pi += 1;
        } else if let Some((sp, sn)) = star {
            pi = sp + 1;
            ni = sn + 1;
            star = Some((sp, sn + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}
