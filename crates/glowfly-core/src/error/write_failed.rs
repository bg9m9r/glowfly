//! Why a write to the working tree or the index failed, as the file, working-tree and stack
//! push results carry it (#367): libgit2's, git's or the system's own words kept as they came,
//! Glowfly's own causes as variants the model words (state/error_words/typed.rs).

use super::Error;
use std::fmt;

/// Why a write to the working tree or index (stage, unstage, discard, a conflict change), a
/// revert, a restore from a commit, a working-tree file operation (new, rename, delete) or a
/// stack push failed.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteFailed {
    /// libgit2's, git's or the system's own words (what `Error::Git` / `Error::Io` said).
    Said { said: String },
    /// A partial clone hasn't downloaded contents the write needs.
    MissingContent { path: String, remote: String },
    /// The write's runner worker panicked: which one, and the panic's own words.
    Panicked { effect: String, panic: String },
    /// Any other error, in the words it has always shown.
    Other { said: String },
}

impl From<Error> for WriteFailed {
    fn from(e: Error) -> Self {
        match e {
            Error::Git(e) => WriteFailed::Said { said: e.to_string() },
            Error::Io(e) => WriteFailed::Said { said: e.to_string() },
            Error::MissingContent(m) => WriteFailed::MissingContent { path: m.path, remote: m.remote },
            other => WriteFailed::Other { said: other.to_string() },
        }
    }
}

impl From<git2::Error> for WriteFailed {
    fn from(e: git2::Error) -> Self {
        WriteFailed::Said { said: e.to_string() }
    }
}

impl From<std::io::Error> for WriteFailed {
    fn from(e: std::io::Error) -> Self {
        WriteFailed::Said { said: e.to_string() }
    }
}

/// The English form these errors have always shown, for tracing and tests; users see
/// `typed::write`'s words in their language.
impl fmt::Display for WriteFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteFailed::Said { said } | WriteFailed::Other { said } => f.write_str(said),
            WriteFailed::MissingContent { path, remote } => {
                write!(f, "the contents of {path} haven't been downloaded from {remote} yet")
            }
            WriteFailed::Panicked { effect, panic } => write!(f, "Glowfly hit an internal error ({effect}): {panic}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partial::MissingContent;

    #[test]
    fn git_and_io_errors_keep_their_own_words() {
        let shown = Error::Git(git2::Error::from_str("index is locked")).to_string();
        assert_eq!(WriteFailed::from(Error::Git(git2::Error::from_str("index is locked"))), WriteFailed::Said { said: shown.clone() });
        assert_eq!(WriteFailed::from(git2::Error::from_str("index is locked")), WriteFailed::Said { said: shown });
        let io = || std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access is denied");
        let shown = Error::Io(io()).to_string();
        assert_eq!(WriteFailed::from(Error::Io(io())), WriteFailed::Said { said: shown.clone() });
        assert_eq!(WriteFailed::from(io()), WriteFailed::Said { said: shown });
    }

    #[test]
    fn missing_content_keeps_path_and_remote() {
        let m = || MissingContent { oids: vec![], remote: "origin".into(), path: "big.bin".into() };
        let e = WriteFailed::from(Error::MissingContent(m()));
        assert_eq!(e, WriteFailed::MissingContent { path: "big.bin".into(), remote: "origin".into() });
        assert_eq!(e.to_string(), Error::MissingContent(m()).to_string());
    }

    #[test]
    fn other_errors_keep_their_old_text() {
        for e in [
            Error::Other("could not write the merged file".into()),
            Error::Unsupported("the diff changed".into()),
            Error::NotFound("no changes in a.txt".into()),
            Error::WouldOverwrite("a.txt has changes only the working tree holds".into()),
        ] {
            let shown = e.to_string();
            let typed = WriteFailed::from(e);
            assert_eq!(typed, WriteFailed::Other { said: shown.clone() });
            assert_eq!(typed.to_string(), shown);
        }
    }

    #[test]
    fn panicked_displays_as_before() {
        let e = WriteFailed::Panicked { effect: "local-op".into(), panic: "boom".into() };
        assert_eq!(e.to_string(), "Glowfly hit an internal error (local-op): boom");
    }
}
