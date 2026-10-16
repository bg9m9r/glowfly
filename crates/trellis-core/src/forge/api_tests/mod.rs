//! Each service's API calls against a mock server (see `mock`): the requests sent (URLs,
//! credentials, headers, bodies) and how answers, errors and links in them are handled.

use super::mock::{Mock, Reply, Request};
use super::*;
use crate::error::Error;

mod azure;
mod bitbucket;
mod forks;
mod github;
mod gitlab;
mod paging;
mod user_repos;

fn remote(url: &str) -> Target {
    parse_remote(url, &[]).unwrap_or_else(|| panic!("unrecognized remote {url}"))
}

fn token(provider: Provider) -> Auth {
    Auth::from_token(provider, None, "tok", "test")
}

/// The one request whose route starts with `prefix`.
fn find(m: &Mock, prefix: &str) -> Request {
    let found: Vec<Request> = m.requests().into_iter().filter(|r| r.route().starts_with(prefix)).collect();
    assert_eq!(found.len(), 1, "{prefix}: {:?}", m.requests().iter().map(Request::route).collect::<Vec<_>>());
    found.into_iter().next().unwrap()
}

fn routes(m: &Mock) -> Vec<String> {
    m.requests().iter().map(Request::route).collect()
}

/// A failed call's error, for a server that answers everything with `status` and `body`.
fn error_for(status: u16, body: &'static str, call: impl FnOnce() -> crate::error::Result<()>) -> Error {
    let _m = Mock::start(move |_| Reply::status(status, body));
    call().unwrap_err()
}

#[test]
fn errors_carry_the_servers_message_by_kind() {
    let t = remote("https://github.com/o/r");
    let auth = token(Provider::GitHub);
    let call = || pull_requests(&t, &auth).map(drop);
    let e = error_for(401, r#"{"message":"Bad credentials"}"#, call);
    assert!(e.is_auth() && e.to_string().contains("\u{201c}Bad credentials\u{201d}"), "{e}");
    assert!(matches!(error_for(403, r#"{"message":"rate limited"}"#, call), Error::Forbidden(m) if m.contains("rate limited")));
    assert!(error_for(404, r#"{"message":"Not Found"}"#, call).is_not_found());
    let e = error_for(502, "<html>\u{1b}[31mbad gateway</html>", call);
    assert!(matches!(&e, Error::Http { status: 502, message } if message.contains("bad gateway") && !message.contains('\u{1b}')), "{e}");
    // GitLab and Azure DevOps put their messages in other fields.
    let gl = remote("https://gitlab.com/g/app");
    let e = error_for(400, r#"{"error":"invalid_token"}"#, || pull_requests(&gl, &token(Provider::GitLab)).map(drop));
    assert!(e.to_string().contains("invalid_token"), "{e}");
    let az = remote("https://dev.azure.com/org/proj/_git/app");
    let e = error_for(400, r#"{"message":"TF401019: no such repository"}"#, || pull_requests(&az, &token(Provider::AzureDevOps)).map(drop));
    assert!(e.to_string().contains("TF401019"), "{e}");
}

#[test]
fn an_html_answer_is_a_sign_in_page() {
    let _m = Mock::start(|_| Reply::text("<html>sign in</html>"));
    let t = remote("https://github.com/o/r");
    assert!(pull_requests(&t, &token(Provider::GitHub)).unwrap_err().is_auth());
}
