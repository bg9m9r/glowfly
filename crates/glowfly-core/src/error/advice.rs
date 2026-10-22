//! A hosting service's refusal in plain words: one sentence that says what went wrong and what
//! to do, from the text of the [`super::Error`] the call returned (its `Display`, as the runner
//! sends it to the app). The service's own words stay available to show under Details.

/// What the failed call was doing, for the sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask<'a> {
    /// Creating the repository `name` on `owner` (an account or organisation).
    CreateRepo { owner: &'a str, name: &'a str },
    /// Listing the accounts and organisations the signed-in user can create repositories on.
    Owners,
}

/// One sentence that says what went wrong with a call to `service` ("GitHub") and what to do,
/// from the call's error text; None when the text isn't one Glowfly recognises (show it as it
/// is then).
pub fn service(service: &str, ask: Ask, raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    if let Ask::CreateRepo { owner, name } = ask
        && lower.contains("already exists")
    {
        return Some(format!("A repository called {name} already exists on {owner}. Choose another name."));
    }
    if raw.starts_with("authentication failed: ") {
        return Some(format!(
            "{service} didn't accept your sign-in. Sign in again, or reconnect the account in Preferences > Integrations."
        ));
    }
    let denied = raw.starts_with("access denied: ");
    if (denied && lower.contains("rate limit")) || raw.starts_with("HTTP 429") {
        return Some(format!("{service} is limiting requests from your account. Wait a few minutes, then try again."));
    }
    if denied {
        return Some(match ask {
            Ask::CreateRepo { owner, .. } => format!(
                "Your {service} account can't create repositories on {owner}. Choose another owner, or ask an owner of {owner} for access."
            ),
            Ask::Owners => {
                format!("Your {service} account didn't allow this. Reconnect it in Preferences > Integrations, then try again.")
            }
        });
    }
    if let Ask::CreateRepo { owner, .. } = ask
        && raw.starts_with("not found: ")
    {
        return Some(format!("{service} can't find {owner} for your account. Choose another owner."));
    }
    if raw.starts_with("network error: ") {
        return Some(format!("Glowfly couldn't reach {service}. Check your network connection, then try again."));
    }
    if raw.starts_with("HTTP 5") {
        return Some(format!("{service} had a problem on its side. Try again in a few minutes."));
    }
    None
}

#[cfg(test)]
#[path = "advice_tests.rs"]
mod tests;
