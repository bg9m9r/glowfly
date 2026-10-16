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
