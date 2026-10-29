//! A hosting service's refusal in plain words: one sentence, in the user's language, that says
//! what went wrong and what to do, from the text of the [`super::Error`] the call returned (its
//! `Display`, as the runner sends it to the app). The service's own words stay available to
//! show under Details or in the log.

use crate::i18n::Lang;

/// What the failed call was doing, for the sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask<'a> {
    /// Creating the repository `name` on `owner` (an account or organisation).
    CreateRepo { owner: &'a str, name: &'a str },
    /// Listing the accounts and organisations the signed-in user can create repositories on.
    Owners,
    /// Finding, listing or creating a fork of a repository.
    Fork,
    /// A change in a repository: creating a pull request or an issue, commenting, reviewing,
    /// merging.
    Change,
    /// Reading from the service: pull request and issue lists, the dashboard, the signed-in
    /// account.
    Read,
}

/// One sentence that says what went wrong with a call to `service` ("GitHub") and what to do,
/// from the call's error text; None when the text isn't one Glowfly recognises (show it as it
/// is then).
pub fn service(t: Lang, service: &str, ask: Ask, raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    let svc: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
    // Glowfly's own explanation of a refused sign-in or request (core's forge code writes many,
    // such as a Trello token that can only read) already says what to do; the generic sentence
    // replaces only the service's words. Creating a repository keeps its sentences either way.
    let own_words = !matches!(ask, Ask::CreateRepo { .. } | Ask::Owners) && glowfly_explained(raw);
    if let Ask::CreateRepo { owner, name } = ask
        && lower.contains("already exists")
    {
        return Some(t.trf("service_errors.repo_exists", &[("name", &name), ("owner", &owner)]));
    }
    if raw.starts_with("authentication failed: ") && !own_words {
        return Some(t.trf("service_errors.sign_in", svc));
    }
    let denied = raw.starts_with("access denied: ");
    if (denied && lower.contains("rate limit")) || raw.starts_with("HTTP 429") {
        return Some(t.trf("service_errors.rate_limited", svc));
    }
    if denied && !own_words {
        return Some(match ask {
            Ask::CreateRepo { owner, .. } => t.trf("service_errors.denied.create_repo", &[("service", &service), ("owner", &owner)]),
            Ask::Fork => t.trf("service_errors.denied.fork", svc),
            Ask::Change => t.trf("service_errors.denied.change", svc),
            Ask::Owners | Ask::Read => t.trf("service_errors.denied", svc),
        });
    }
    // What wasn't found is only certain for these; elsewhere it may be an issue or a pull
    // request, which the service's own words name.
    if raw.starts_with("not found: ") {
        match ask {
            Ask::CreateRepo { owner, .. } => {
                return Some(t.trf("service_errors.not_found.owner", &[("service", &service), ("owner", &owner)]));
            }
            Ask::Fork => return Some(t.trf("service_errors.not_found.repo", svc)),
            Ask::Owners | Ask::Change | Ask::Read => {}
        }
    }
    if raw.starts_with("network error: ") {
        return Some(t.trf("service_errors.network", svc));
    }
    if raw.starts_with("HTTP 5") {
        return Some(t.trf("service_errors.server", svc));
    }
    None
}

/// Whether the text after `raw`'s kind prefix is Glowfly's own explanation rather than the
/// service's words: those come quoted ("the server said …"), empty, or as a bare status code.
fn glowfly_explained(raw: &str) -> bool {
    let inner = super::plain_message(raw).trim();
    !(inner.is_empty() || inner.starts_with("the server said") || inner.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
#[path = "advice_tests.rs"]
mod tests;
