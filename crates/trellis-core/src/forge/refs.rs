//! Issue references in commit messages, PR text and branch names.

use super::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RefStyle {
    /// `AB#123`
    AzureBoards,
    /// `#123`
    Hash,
    /// `PROJ-123`
    Jira,
}

pub fn mention(style: RefStyle, key: &str) -> String {
    match style {
        RefStyle::AzureBoards => format!("AB#{key}"),
        RefStyle::Hash => format!("#{key}"),
        RefStyle::Jira => key.to_owned(),
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
        RefStyle::Jira => {
            let mut i = 0;
            while i < b.len() {
                let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
                if boundary && b[i].is_ascii_uppercase() {
                    let mut j = i + 1;
                    while j < b.len() && (b[j].is_ascii_uppercase() || b[j].is_ascii_digit() || b[j] == b'_') {
                        j += 1;
                    }
                    if j < b.len() && b[j] == b'-' && j - i >= 2 {
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

/// An issue key encoded at the start of a branch's last path segment, e.g.
/// `feature/1234-login-page`, `1234_fix`, or `bugfix/PROJ-12-crash`.
pub fn branch_issue(style: RefStyle, branch: &str) -> Option<String> {
    let last = branch.rsplit('/').next()?;
    match style {
        RefStyle::Jira => issue_refs(RefStyle::Jira, last).into_iter().next().filter(|k| last.starts_with(k.as_str())),
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
