//! Starting a code comment on lines of a pull request's diff, either alone (a single comment)
//! or in the signed-in user's pending review, which is then finished with `submit_review`. Only
//! GitHub and GitHub Enterprise Server; every other service answers `Error::Unsupported`
//! without sending anything.

use super::super::{Auth, Provider, Target};
use super::{LineAnchor, ReviewThread, github_new_comment, thread, unsupported};
use crate::error::{Error, Result};

/// Whether code comments can be started on diff lines inside Glowfly for this service. Kept
/// apart from `reviews_in_app` so a service can have in-app reviews before line comments.
pub fn line_comments_in_app(p: Provider) -> bool {
    matches!(p, Provider::GitHub | Provider::GitHubEnterprise)
}

/// A comment to start on lines of the changes up to `head_sha` (the head the lines belong to).
#[derive(Clone, Debug, PartialEq)]
pub struct NewComment {
    pub head_sha: String,
    pub anchor: LineAnchor,
    pub body: String,
}

/// The thread a new comment started.
#[derive(Clone, Debug, PartialEq)]
pub struct NewThread {
    pub thread: ReviewThread,
    /// The pending review it went into; `None` for a single comment.
    pub pending_review: Option<u64>,
}

/// What a single comment's thread is called until the page is read again (REST answers with
/// the comment, not its thread).
pub const NEW_THREAD_PREFIX: &str = "new:";

/// The provisional id of the thread single comment `comment` started.
pub fn new_thread_id(comment: u64) -> String {
    format!("{NEW_THREAD_PREFIX}{comment}")
}

/// The comment a provisional thread id (`new_thread_id`) names; `None` for a service's id.
pub fn new_thread_comment(id: &str) -> Option<u64> {
    id.strip_prefix(NEW_THREAD_PREFIX)?.parse().ok()
}

const EMPTY: &str = "Write a comment first";

/// The text sent: without leading blank lines or trailing whitespace.
fn body_text(s: &str) -> String {
    let start = s.split_inclusive('\n').take_while(|l| l.trim().is_empty()).map(str::len).sum::<usize>();
    s[start..].trim_end().to_owned()
}

/// The text to send, or why nothing is sent.
fn checked_body(t: &Target, c: &NewComment) -> Result<String> {
    if !line_comments_in_app(t.provider) {
        return Err(unsupported(t));
    }
    let body = body_text(&c.body);
    if body.is_empty() {
        return Err(Error::Other(EMPTY.into()));
    }
    Ok(body)
}

fn checked(t: &Target, mut n: NewThread) -> NewThread {
    n.thread = thread::checked_thread(t, n.thread);
    n
}

/// Posts `c` on pull request `id` as a single comment; returns its thread, with a provisional
/// id (`new_thread_id`) until the page is read again.
pub fn post_line_comment(t: &Target, auth: &Auth, id: u64, c: &NewComment) -> Result<NewThread> {
    let body = checked_body(t, c)?;
    github_new_comment::single(t, auth, id, c, &body).map(|n| checked(t, n))
}

/// Adds `c` to the signed-in user's pending review of pull request `id` (`pending`, when the
/// page knew it), starting a review when there is none; returns the thread and the review.
pub fn add_review_comment(t: &Target, auth: &Auth, id: u64, pending: Option<u64>, c: &NewComment) -> Result<NewThread> {
    let body = checked_body(t, c)?;
    github_new_comment::to_review(t, auth, id, pending, c, &body).map(|n| checked(t, n))
}

#[cfg(test)]
mod tests {
    use super::super::super::mock::{Mock, Reply};
    use super::*;
    use crate::diff_rows::LineSide;

    fn comment(body: &str) -> NewComment {
        let anchor = LineAnchor { path: "a.rs".into(), line: 3, side: LineSide::New, start: None };
        NewComment { head_sha: "abc".into(), anchor, body: body.into() }
    }

    fn target(p: Provider) -> Target {
        Target { provider: p, host: "example.com".into(), owner: "o".into(), project: "p".into(), repo: "r".into() }
    }

    #[test]
    fn line_comments_in_app_only_for_github() {
        for p in Provider::ALL {
            assert_eq!(line_comments_in_app(p), matches!(p, Provider::GitHub | Provider::GitHubEnterprise), "{p:?}");
        }
    }

    #[test]
    fn other_services_send_nothing() {
        let m = Mock::start(|_| Reply::not_found());
        for p in [Provider::GitLab, Provider::Bitbucket, Provider::AzureDevOps, Provider::JiraCloud] {
            let (t, auth) = (target(p), Auth::from_token(p, None, "tok", "test"));
            let errors = [
                post_line_comment(&t, &auth, 1, &comment("hi")).unwrap_err(),
                add_review_comment(&t, &auth, 1, None, &comment("hi")).unwrap_err(),
            ];
            for e in errors {
                assert!(matches!(&e, Error::Unsupported(m) if m.contains(p.name())), "{p:?}: {e}");
            }
        }
        assert!(m.requests().is_empty());
    }

    #[test]
    fn an_empty_body_is_refused_without_a_request() {
        let m = Mock::start(|_| Reply::not_found());
        let (t, auth) = (target(Provider::GitHub), Auth::from_token(Provider::GitHub, None, "tok", "test"));
        for body in ["", "  \n\t\n "] {
            let e = post_line_comment(&t, &auth, 1, &comment(body)).unwrap_err();
            assert!(matches!(&e, Error::Other(m) if m == EMPTY), "{e}");
            let e = add_review_comment(&t, &auth, 1, Some(9), &comment(body)).unwrap_err();
            assert!(matches!(&e, Error::Other(m) if m == EMPTY), "{e}");
        }
        assert!(m.requests().is_empty());
    }

    #[test]
    fn body_text_drops_leading_blank_lines_and_trailing_space() {
        assert_eq!(body_text("\n  \n  indented\nnext  \n\n"), "  indented\nnext");
        assert_eq!(body_text("plain"), "plain");
    }

    #[test]
    fn provisional_thread_ids_round_trip() {
        assert_eq!(new_thread_id(901), "new:901");
        assert_eq!(new_thread_comment(&new_thread_id(901)), Some(901));
        for other in ["T_kwDO", "new:", "new:x", "901"] {
            assert_eq!(new_thread_comment(other), None, "{other}");
        }
    }
}
