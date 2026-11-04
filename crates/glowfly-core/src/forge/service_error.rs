//! A failed call to a hosting service as a result message carries it (#367): which kind of
//! failure, with the service's (or the system's) own words, rather than English text. The model
//! words it in the user's language (`state/error_words/typed.rs`). Never holds a token.

use crate::error::Error;

/// Why signing in to a hosting service, or a call to it, failed. `said` is the service's,
/// libgit2's or the system's own words; Glowfly's forge code writes some of them itself (a
/// Trello token that can only read, say).
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceError {
    /// Missing, rejected or expired credentials.
    Auth { said: String },
    /// The account isn't allowed to do this.
    Forbidden { said: String },
    /// What the call asked for isn't there.
    NotFound { said: String },
    /// Any other HTTP refusal, with its status.
    Http { status: u16, message: String },
    /// The service couldn't be reached.
    Network { said: String },
    /// The service answered with something Glowfly couldn't read.
    Decode { said: String },
    /// The OS credential store failed.
    Keyring { said: String },
    /// Anything else, in its own words.
    Other { said: String },
}

impl From<Error> for ServiceError {
    fn from(e: Error) -> Self {
        match e {
            Error::Auth(said) => Self::Auth { said },
            Error::Forbidden(said) => Self::Forbidden { said },
            Error::NotFound(said) => Self::NotFound { said },
            Error::Http { status, message } => Self::Http { status, message },
            Error::Network(said) => Self::Network { said },
            Error::Decode(said) => Self::Decode { said },
            Error::Keyring(k) => Self::Keyring { said: k.to_string() },
            other => Self::Other { said: other.to_string() },
        }
    }
}

/// The English form `Error` gives, which `error::advice::service` reads to find a sentence.
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth { said } => write!(f, "authentication failed: {said}"),
            Self::Forbidden { said } => write!(f, "access denied: {said}"),
            Self::NotFound { said } => write!(f, "not found: {said}"),
            Self::Http { status, message } => write!(f, "HTTP {status}: {message}"),
            Self::Network { said } => write!(f, "network error: {said}"),
            Self::Decode { said } => write!(f, "unexpected response: {said}"),
            Self::Keyring { said } => write!(f, "credential store: {said}"),
            Self::Other { said } => f.write_str(said),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_kind_and_reads_as_the_error_did() {
        let cases = [
            (Error::Auth("the token expired".into()), ServiceError::Auth { said: "the token expired".into() }),
            (Error::Forbidden("rate limit".into()), ServiceError::Forbidden { said: "rate limit".into() }),
            (Error::NotFound("acme".into()), ServiceError::NotFound { said: "acme".into() }),
            (Error::Http { status: 502, message: "bad gateway".into() }, ServiceError::Http { status: 502, message: "bad gateway".into() }),
            (Error::Network("timed out".into()), ServiceError::Network { said: "timed out".into() }),
            (Error::Decode("expected a list".into()), ServiceError::Decode { said: "expected a list".into() }),
            (Error::Keyring(keyring::Error::NoEntry), ServiceError::Keyring { said: keyring::Error::NoEntry.to_string() }),
            (Error::Other("gcm said no".into()), ServiceError::Other { said: "gcm said no".into() }),
            (Error::NotARepository("x".into()), ServiceError::Other { said: "x is not a git repository".into() }),
        ];
        for (e, want) in cases {
            let english = e.to_string();
            let typed = ServiceError::from(e);
            assert_eq!(typed, want);
            assert_eq!(typed.to_string(), english, "{typed:?} reads as the error did");
        }
    }
}
