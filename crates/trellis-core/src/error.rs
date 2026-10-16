//! Typed errors for everything in `trellis-core`, so callers can react to the kind of failure
//! (for example, prompt to sign in on `Auth`) rather than inspecting messages.

/// Errors returned by repository access and hosting-service calls.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0} is not a git repository")]
    NotARepository(String),
    /// git refuses the repository in this folder because another user owns it, until it is
    /// listed in `safe.directory` (see `safe_directory`).
    #[error("git won't open {} because another user owns it", crate::native_path::shown(.0))]
    NotOwned(std::path::PathBuf),
    /// Missing, rejected or expired credentials.
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("access denied: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error("credential store: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("{0}")]
    Unsupported(String),
    /// A write was refused because it would destroy working-tree contents nothing else holds;
    /// the text says why. Asking the user and retrying with overwriting allowed is the remedy.
    #[error("{0}")]
    WouldOverwrite(String),
    /// The system trash couldn't take a file; nothing was removed. Its problem says whether a
    /// permanent delete is the remedy (no trash) or retrying after fixing the cause.
    #[error("{0}")]
    TrashRefused(crate::repo::TrashRefusal),
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// True when signing in again (or connecting) could fix the problem.
    pub fn is_auth(&self) -> bool {
        matches!(self, Error::Auth(_))
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Decode(e.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_errors() {
        assert!(Error::Auth("expired".into()).is_auth());
        assert!(!Error::Http { status: 500, message: "boom".into() }.is_auth());
        assert!(Error::NotFound("x".into()).is_not_found());
        assert_eq!(Error::Http { status: 502, message: "bad gateway".into() }.to_string(), "HTTP 502: bad gateway");
    }
}
