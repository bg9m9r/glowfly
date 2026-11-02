//! What the trust prompt recognises among a scan's settings and hooks: today only Git LFS's own
//! standard setup (the filter settings and hook scripts `git lfs install` writes). Display
//! only: the prompt marks a recognised row, and nothing here feeds the trust print, `compare`
//! or holding, so what is trusted, compared or held stays the same. The matchers are strict:
//! a value or script that differs in any way, however close, isn't recognised.

use super::{ConfigCommand, Follow};

/// A tool whose standard settings and hooks Glowfly recognises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    GitLfs,
}

/// The most bytes of a hook script read for `hook` (Git LFS's are a few hundred).
pub(super) const MAX_HOOK_BYTES: usize = 4096;

/// The values Git LFS's filter settings take, by setting name.
const LFS_FILTERS: &[(&str, &[&str])] = &[
    ("clean", &["git-lfs clean -- %f"]),
    ("smudge", &["git-lfs smudge -- %f", "git-lfs smudge --skip -- %f"]),
    ("process", &["git-lfs filter-process", "git-lfs filter-process --skip"]),
];

/// The hooks `git lfs install` writes.
const LFS_HOOKS: &[&str] = &["pre-push", "post-checkout", "post-commit", "post-merge"];

/// Whether `c` is one of Git LFS's standard filter settings: `filter.lfs.clean`, `.smudge` or
/// `.process` (section and name in any case, the subsection exactly `lfs`) with one of its
/// standard values (surrounding whitespace aside), while no file the settings name in the
/// working tree (`follows`) is called `git-lfs` or `git-lfs.exe`: git would run that one.
pub fn setting(c: &ConfigCommand, follows: &[Follow]) -> Option<Known> {
    let (section, rest) = c.key.split_once('.')?;
    let (sub, name) = rest.rsplit_once('.')?;
    if !section.eq_ignore_ascii_case("filter") || sub != "lfs" {
        return None;
    }
    let (_, values) = LFS_FILTERS.iter().find(|(n, _)| name.eq_ignore_ascii_case(n))?;
    if !values.contains(&c.value.trim()) {
        return None;
    }
    let shadowed = follows.iter().any(|f| f.path.eq_ignore_ascii_case("git-lfs") || f.path.eq_ignore_ascii_case("git-lfs.exe"));
    (!shadowed).then_some(Known::GitLfs)
}

/// Whether the hook `name` holding `bytes` is the script `git lfs install` writes for it: one
/// of its four hooks, valid UTF-8, and (CRLF read as LF, trailing whitespace and blank lines
/// dropped) exactly `#!/bin/sh`, the check that git-lfs is installed with one of its two
/// message forms, and `git lfs <name> "$@"`.
pub fn hook(name: &str, bytes: &[u8]) -> Option<Known> {
    if !LFS_HOOKS.contains(&name) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?.replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').map(str::trim_end).filter(|l| !l.is_empty()).collect();
    let [shebang, check, run] = lines.as_slice() else { return None };
    let ok = *shebang == "#!/bin/sh" && is_lfs_check(check) && *run == format!("git lfs {name} \"$@\"");
    ok.then_some(Known::GitLfs)
}

/// `command -v git-lfs >/dev/null 2>&1 || { <say M>; exit 2; }`, where saying M is
/// `printf >&2 "\n%s\n\n" "M"` or `echo >&2 "M"` and M is plain (`plain_message`).
fn is_lfs_check(line: &str) -> bool {
    let Some(say) = line.strip_prefix("command -v git-lfs >/dev/null 2>&1 || { ").and_then(|l| l.strip_suffix("; exit 2; }")) else {
        return false;
    };
    let message = say.strip_prefix("printf >&2 \"\\n%s\\n\\n\" \"").or_else(|| say.strip_prefix("echo >&2 \""));
    message.and_then(|m| m.strip_suffix('"')).is_some_and(plain_message)
}

/// A message the shell prints as written: no `"`, `$` or backtick, and no `\` but in `\n`.
fn plain_message(m: &str) -> bool {
    let mut chars = m.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' | '$' | '`' => return false,
            '\\' if chars.next() != Some('n') => return false,
            _ => {}
        }
    }
    true
}

#[cfg(test)]
#[path = "known_tests.rs"]
mod tests;
