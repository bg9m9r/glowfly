//! Posting a comment on an issue, for every tracker Glowfly shows issues from: the draft is
//! normalised and checked against the service's length limit here, once, before anything is
//! sent; each service posts it in its own `*_issues.rs`. Bitbucket Data Center has no issue
//! tracker of its own, so it answers `Error::Unsupported` without sending anything. The link in
//! the answer is checked here.

use super::issue_details::{issues_in_app, unsupported};
use super::{Auth, Entry, Provider, Target, links, service};
use crate::error::{Error, Result};

/// The text a draft is sent as: leading blank lines are dropped, trailing whitespace is
/// trimmed and line ends become `\n`. The first text line keeps its indentation, so an
/// indented code block still works.
pub fn comment_text(draft: &str) -> String {
    let text = draft.replace("\r\n", "\n");
    let mut rest = text.as_str();
    while let Some(i) = rest.find('\n') {
        if !rest[..i].trim().is_empty() {
            break;
        }
        rest = &rest[i + 1..];
    }
    rest.trim_end().to_owned()
}

/// The longest comment the service takes, in characters.
pub fn max_comment(p: Provider) -> usize {
    match p {
        Provider::GitLab | Provider::GitLabSelfManaged => 1_000_000,
        Provider::Trello => 16_384,
        _ if p.is_jira() => 32_767,
        _ => 65_536,
    }
}

/// `n` with its digits in groups of three: 16,384.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Why the service would refuse `draft` as a comment: blank, or too long.
pub fn comment_problem(p: Provider, draft: &str) -> Option<String> {
    let text = comment_text(draft);
    let max = max_comment(p);
    if text.is_empty() {
        Some("Write a comment first".into())
    } else if text.chars().count() > max {
        Some(format!("Comments on {} can have at most {} characters", p.name(), grouped(max)))
    } else {
        None
    }
}

/// `body` quoted for a reply by `author`: "> " lines under "<author> wrote:" (a blank line is
/// ">"), or a `{quote}` block on Jira Data Center, whose markup is wiki text. Without an author
/// the "wrote:" line is left out; an empty body gives an empty quote.
pub fn quote_comment(p: Provider, author: &str, body: &str) -> String {
    let body = body.replace("\r\n", "\n");
    let body = body.trim();
    if body.is_empty() {
        return String::new();
    }
    let author = author.trim();
    let mut out = if author.is_empty() { String::new() } else { format!("{author} wrote:\n") };
    if p == Provider::JiraDataCenter {
        out.push_str(&format!("{{quote}}\n{body}\n{{quote}}"));
    } else {
        let lines: Vec<String> = body.lines().map(|l| if l.trim().is_empty() { ">".to_owned() } else { format!("> {l}") }).collect();
        out.push_str(&lines.join("\n"));
    }
    out
}

/// Posts `body` as a comment on issue `key` and returns the comment as the service has it. A
/// blank or over-long body is refused before anything is sent.
pub fn post_issue_comment(t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
    if !issues_in_app(t.provider) {
        return Err(unsupported(t));
    }
    if let Some(why) = comment_problem(t.provider, body) {
        return Err(Error::Other(why));
    }
    let mut e = service(t.provider).post_issue_comment(t, auth, key, &comment_text(body))?;
    e.url = links::web_url(t, std::mem::take(&mut e.url));
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_text_drops_leading_blank_lines_and_trailing_space() {
        assert_eq!(comment_text("\n  \n  Looks good  \n\n"), "  Looks good");
        assert_eq!(comment_text("\r\n    code\r\nnext\r\n"), "    code\nnext");
        assert_eq!(comment_text("One\n\n  two \t"), "One\n\n  two");
        assert_eq!(comment_text("  \n "), "");
        assert_eq!(comment_text(""), "");
    }

    #[test]
    fn limits_per_service() {
        use Provider::*;
        for (p, max) in [
            (GitHub, 65_536),
            (GitHubEnterprise, 65_536),
            (GitLab, 1_000_000),
            (GitLabSelfManaged, 1_000_000),
            (JiraCloud, 32_767),
            (JiraDataCenter, 32_767),
            (Trello, 16_384),
            (AzureDevOps, 65_536),
            (Bitbucket, 65_536),
        ] {
            assert_eq!(max_comment(p), max, "{p:?}");
        }
    }

    #[test]
    fn problems_name_the_limit() {
        assert_eq!(comment_problem(Provider::GitHub, "  \n ").as_deref(), Some("Write a comment first"));
        let long = "x".repeat(16_385);
        assert_eq!(comment_problem(Provider::Trello, &long).as_deref(), Some("Comments on Trello can have at most 16,384 characters"));
        assert_eq!(comment_problem(Provider::Trello, &long[1..]), None, "exactly the limit");
        assert_eq!(comment_problem(Provider::GitHub, "Thanks"), None);
        assert_eq!(
            (grouped(16_384), grouped(999), grouped(1_000_000), grouped(0)),
            ("16,384".into(), "999".into(), "1,000,000".into(), "0".into())
        );
    }

    #[test]
    fn quote_comment_per_service() {
        assert_eq!(quote_comment(Provider::GitHub, " ada ", "One\r\n\r\nTwo\n"), "ada wrote:\n> One\n>\n> Two");
        assert_eq!(quote_comment(Provider::JiraDataCenter, "ada", "One\n"), "ada wrote:\n{quote}\nOne\n{quote}");
        assert_eq!(quote_comment(Provider::GitLab, "", "One"), "> One", "no author, no wrote line");
        assert_eq!(quote_comment(Provider::JiraDataCenter, "", "One"), "{quote}\nOne\n{quote}");
        assert_eq!(quote_comment(Provider::GitHub, "ada", " \r\n "), "", "an empty body");
    }

    #[test]
    fn post_refuses_blank_and_unsupported_without_sending() {
        use super::super::mock::{Mock, Reply, Request};
        let m = Mock::start(|_| Reply::json("{}"));
        let gh =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        let auth = Auth::from_token(Provider::GitHub, None, "tok", "test");
        let e = post_issue_comment(&gh, &auth, "9", "  \n").unwrap_err();
        assert!(matches!(&e, Error::Other(m) if m == "Write a comment first"), "{e}");
        let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), project: "p".into(), ..gh };
        let e = post_issue_comment(&dc, &Auth::from_token(Provider::BitbucketDataCenter, None, "tok", "test"), "1", "Hi").unwrap_err();
        assert!(matches!(&e, Error::Unsupported(m) if m.contains("use Jira")), "{e}");
        assert!(m.requests().iter().map(Request::route).next().is_none(), "nothing was sent");
    }
}
