//! Issue references in commit messages, PR text and branch names.

use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RefStyle {
    /// `AB#123`
    AzureBoards,
    /// `#123`
    Hash,
    /// `PROJ-123`
    Jira,
    /// trello.com/c/<shortLink> card links
    Trello,
}

pub fn mention(style: RefStyle, key: &str) -> String {
    match style {
        RefStyle::AzureBoards => format!("AB#{key}"),
        RefStyle::Hash => format!("#{key}"),
        RefStyle::Jira | RefStyle::Trello => key.to_owned(),
    }
}

fn push_unique(out: &mut Vec<String>, k: String) {
    if !out.contains(&k) {
        out.push(k);
    }
}

/// Extracts issue mentions from free text in the tracker's syntax.
pub fn issue_refs(style: RefStyle, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    match style {
        RefStyle::AzureBoards | RefStyle::Hash => {
            for (i, _) in text.match_indices('#') {
                let digits: String = text[i + 1..].chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    continue;
                }
                let prefix_ok = match style {
                    RefStyle::AzureBoards => {
                        i >= 2 && text[..i].to_ascii_lowercase().ends_with("ab") && (i == 2 || !b[i - 3].is_ascii_alphanumeric())
                    }
                    _ => i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'&'),
                };
                if prefix_ok {
                    push_unique(&mut out, digits.trim_start_matches('0').to_owned());
                }
            }
        }
        RefStyle::Trello => trello_links(text, &mut out),
        RefStyle::Jira => {
            let mut i = 0;
            while i < b.len() {
                let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
                if boundary && b[i].is_ascii_uppercase() {
                    let mut j = i + 1;
                    while j < b.len() && (b[j].is_ascii_uppercase() || b[j].is_ascii_digit() || b[j] == b'_') {
                        j += 1;
                    }
                    // Jira numbers issues from 1 and never pads them with zeros.
                    if j + 1 < b.len() && b[j] == b'-' && b[j + 1] != b'0' && j - i >= 2 {
                        let mut k = j + 1;
                        while k < b.len() && b[k].is_ascii_digit() {
                            k += 1;
                        }
                        let end_ok = k == b.len() || !b[k].is_ascii_alphanumeric();
                        if k > j + 1 && end_ok {
                            push_unique(&mut out, text[i..k].to_owned());
                            i = k;
                            continue;
                        }
                    }
                    i = j;
                    continue;
                }
                i += 1;
            }
        }
    }
    out.retain(|k| !k.is_empty());
    out
}

/// Card ids from `trello.com/c/<id>` links (with or without `https://` or `www.`). The host
/// must start at a boundary, so `eviltrello.com` and `a.trello.com` don't count.
fn trello_links(text: &str, out: &mut Vec<String>) {
    const LINK: &str = "trello.com/c/";
    let lower = text.to_ascii_lowercase(); // same byte offsets as `text`
    let b = lower.as_bytes();
    let boundary = |at: usize| at == 0 || !(b[at - 1].is_ascii_alphanumeric() || matches!(b[at - 1], b'.' | b'-' | b'_'));
    for (i, _) in lower.match_indices(LINK) {
        let host_ok = boundary(i) || (i >= 4 && &b[i - 4..i] == b"www." && boundary(i - 4));
        let start = i + LINK.len();
        let len = b[start..].iter().take_while(|c| c.is_ascii_alphanumeric()).count();
        if host_ok && len == 8 {
            push_unique(out, text[start..start + 8].to_owned());
        }
    }
}

/// A Trello card id at the start of a branch's last segment: 8 letters or digits, then the
/// end, `-` or `_`. Plain words and dates (`frontend-x`, `20261016-release`) look the same, so
/// the id must not be all digits and must hold a digit or an uppercase letter. An all-lowercase
/// id (about 0.1% of random ids) is missed, and a false match is skipped as an unknown card.
fn trello_branch(last: &str) -> Option<String> {
    let b = last.as_bytes();
    let id = b.get(..8)?;
    let ok = id.iter().all(u8::is_ascii_alphanumeric)
        && matches!(b.get(8), None | Some(b'-') | Some(b'_'))
        && !id.iter().all(u8::is_ascii_digit)
        && id.iter().any(|c| c.is_ascii_digit() || c.is_ascii_uppercase());
    ok.then(|| last[..8].to_owned())
}

/// Upper-case words written before a hyphen and number in ordinary text (character sets,
/// hashes, ciphers, standards), never treated as projects unless the site has one by that name.
const NOT_PROJECTS: &[&str] = &[
    "AES", "ANSI", "CP", "CRC", "CVE", "CWE", "DES", "ECMA", "HMAC", "HTTP", "IEC", "IEEE", "ISO", "MD", "PEP", "RFC", "RSA", "SHA", "SSL",
    "TLS", "UCS", "UTF", "WIN", "X86",
];

/// The project part of a Jira key: `PROJ` in `PROJ-12`.
pub fn jira_project(key: &str) -> &str {
    key.rsplit_once('-').map_or(key, |(p, _)| p)
}

/// Whether a Jira-shaped `key` counts as an issue: with a project `filter`, only that project's
/// keys do; else, once the site's `projects` are known, only theirs; else any key whose project
/// isn't a word that is never a project (`UTF-8`, `SHA-256`).
pub fn keep_jira_key(key: &str, filter: &str, projects: Option<&HashSet<String>>) -> bool {
    let project = jira_project(key);
    let filter = filter.trim();
    if !filter.is_empty() {
        return project == filter.to_ascii_uppercase();
    }
    match projects {
        Some(set) => set.contains(project),
        None => !NOT_PROJECTS.contains(&project),
    }
}

/// An issue key encoded at the start of a branch's last path segment, e.g.
/// `feature/1234-login-page`, `1234_fix`, or `bugfix/PROJ-12-crash`.
pub fn branch_issue(style: RefStyle, branch: &str) -> Option<String> {
    let last = branch.rsplit('/').next()?;
    match style {
        RefStyle::Jira => issue_refs(RefStyle::Jira, last).into_iter().next().filter(|k| last.starts_with(k.as_str())),
        RefStyle::Trello => trello_branch(last),
        _ => {
            let digits: String = last.chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() {
                return None;
            }
            match last[digits.len()..].chars().next() {
                None | Some('-') | Some('_') => Some(digits),
                _ => None,
            }
        }
    }
}

/// Branch name for starting work on an issue: `1234-short-title-slug` / `PROJ-12-short-title`.
pub fn branch_name_for(issue: &Issue) -> String {
    let mut slug = String::new();
    for c in issue.title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 40 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() { issue.key.clone() } else { format!("{}-{slug}", issue.key) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jira_numbers_never_start_with_zero() {
        assert_eq!(issue_refs(RefStyle::Jira, "ABC-007 and ABC-0 and ABC-7"), ["ABC-7"]);
        assert_eq!(issue_refs(RefStyle::Jira, "ends with ABC-"), Vec::<String>::new());
    }

    #[test]
    fn keeps_jira_keys_by_filter_projects_or_the_word_list() {
        for word in ["UTF-8", "ISO-8859", "SHA-256", "CVE-2024", "X86-64"] {
            assert!(!keep_jira_key(word, "", None), "{word}");
        }
        assert!(keep_jira_key("PROJ-12", "", None));
        let projects: HashSet<String> = HashSet::from(["UTF".to_owned()]);
        assert!(keep_jira_key("UTF-8", "", Some(&projects)));
        assert!(!keep_jira_key("PROJ-12", "", Some(&projects)));
        assert!(keep_jira_key("PROJ-1", " proj ", None));
        assert!(!keep_jira_key("ABC-1", " proj ", Some(&projects)));
        assert_eq!(jira_project("A_B2-12"), "A_B2");
    }
}
