//! Why a background read of the repository failed, as the graph's, diff's and repository's reads'
//! result messages carry it (#367): libgit2's or the system's own words kept as they came,
//! Glowfly's own causes as variants the model words (state/error_words/typed.rs).

use super::Error;
use std::fmt;

/// Why a read for the graph (a comparison, stacked lists, path search, history index), the
/// diff (hunk patch, unchanged lines, binary sides, conflict, conflict markers, file history) or
/// the repository (a load, the status, a commit's detail, a file diff, a signature check, line
/// counts, blame, the unmerged count, the dashboard scan, an operation preview, the sparse
/// status, a stash's files) failed.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadFailed {
    /// libgit2's or the system's own words (what `Error::Git` / `Error::Io` said).
    Said { said: String },
    /// The folder holds no git repository (`Error::NotARepository`).
    NotARepository { path: String },
    /// git refuses the repository at `root` (as shown) because another user owns it
    /// (`Error::NotOwned`).
    NotOwned { root: String },
    /// A repository load couldn't open `path` (as shown); why.
    OpenFailed { path: String, why: Box<ReadFailed> },
    /// A read's runner worker panicked: which one, and the panic's own words.
    Panicked { effect: String, panic: String },
    /// A partial clone hasn't downloaded contents the read needs.
    MissingContent { path: String, remote: String },
    /// Copy hunk as patch: git no longer produces the hunk on screen.
    DiffChanged,
    /// The unchanged lines: the file's hunks differ from the ones on screen now.
    FileChanged,
    /// The unchanged lines: the old file can't be read in full.
    OldFileUnreadable,
    /// The conflict view: `path` isn't conflicted.
    NotConflicted { path: String },
    /// A folder history was asked for the repository root.
    NoFolder,
    /// A runner worker couldn't start its thread (the system's words).
    NoThread { said: String },
    /// A runner worker panicked; which one.
    Internal { effect: String },
    /// An index build was stopped (the runner's run-at-once path).
    Stopped,
    /// Any other error, in the words it has always shown.
    Other { said: String },
}

impl From<Error> for ReadFailed {
    fn from(e: Error) -> Self {
        match e {
            Error::Git(e) => ReadFailed::Said { said: e.to_string() },
            Error::Io(e) => ReadFailed::Said { said: e.to_string() },
            Error::NotARepository(path) => ReadFailed::NotARepository { path },
            Error::NotOwned(r) => ReadFailed::NotOwned { root: crate::native_path::shown(&r.root) },
            Error::MissingContent(m) => ReadFailed::MissingContent { path: m.path, remote: m.remote },
            other => ReadFailed::Other { said: other.to_string() },
        }
    }
}

impl From<git2::Error> for ReadFailed {
    fn from(e: git2::Error) -> Self {
        ReadFailed::Said { said: e.to_string() }
    }
}

impl From<std::io::Error> for ReadFailed {
    fn from(e: std::io::Error) -> Self {
        ReadFailed::Said { said: e.to_string() }
    }
}

/// The English form these errors have always shown, for tracing and tests; users see
/// `typed::read`'s words in their language.
impl fmt::Display for ReadFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadFailed::Said { said } | ReadFailed::Other { said } => f.write_str(said),
            ReadFailed::NotARepository { path } => write!(f, "{path} is not a git repository"),
            ReadFailed::NotOwned { root } => write!(f, "git won't open {root} because another user owns it"),
            ReadFailed::OpenFailed { path, why } => write!(f, "Could not open {path}: {why}"),
            ReadFailed::Panicked { effect, panic } => write!(f, "Glowfly hit an internal error ({effect}): {panic}"),
            ReadFailed::MissingContent { path, remote } => {
                write!(f, "the contents of {path} haven't been downloaded from {remote} yet")
            }
            ReadFailed::DiffChanged => f.write_str("not found: the diff changed; try again on the updated diff"),
            ReadFailed::FileChanged => f.write_str("not found: the file changed"),
            ReadFailed::OldFileUnreadable => f.write_str("the old file can't be read"),
            ReadFailed::NotConflicted { path } => write!(f, "not found: {path} is not conflicted"),
            ReadFailed::NoFolder => f.write_str("A folder history needs a folder"),
            ReadFailed::NoThread { said } => write!(f, "could not start a thread: {said}"),
            ReadFailed::Internal { effect } => write!(f, "Glowfly hit an internal error ({effect})"),
            ReadFailed::Stopped => f.write_str("stopped"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partial::MissingContent;
    use crate::safe_directory::Refusal;

    #[test]
    fn git_and_io_errors_keep_their_own_words() {
        let git = git2::Error::from_str("object not found");
        let shown = Error::Git(git2::Error::from_str("object not found")).to_string();
        assert_eq!(ReadFailed::from(Error::Git(git)), ReadFailed::Said { said: shown.clone() });
        assert_eq!(ReadFailed::from(git2::Error::from_str("object not found")), ReadFailed::Said { said: shown });
        let io = || std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access is denied");
        let shown = Error::Io(io()).to_string();
        assert_eq!(ReadFailed::from(Error::Io(io())), ReadFailed::Said { said: shown.clone() });
        assert_eq!(ReadFailed::from(io()), ReadFailed::Said { said: shown });
    }

    #[test]
    fn missing_content_keeps_path_and_remote() {
        let m = || MissingContent { oids: vec![], remote: "origin".into(), path: "big.bin".into() };
        let e = ReadFailed::from(Error::MissingContent(m()));
        assert_eq!(e, ReadFailed::MissingContent { path: "big.bin".into(), remote: "origin".into() });
        assert_eq!(e.to_string(), Error::MissingContent(m()).to_string());
    }

    #[test]
    fn other_errors_keep_their_old_text() {
        for e in [
            Error::Other("could not parse the merged file".into()),
            Error::Unsupported("is a folder".into()),
            Error::NotFound("no changes in a.txt".into()),
        ] {
            let shown = e.to_string();
            assert_eq!(ReadFailed::from(e), ReadFailed::Other { said: shown });
        }
    }

    #[test]
    fn not_a_repository_and_not_owned_get_their_own_variants() {
        let shown = Error::NotARepository("/x".into()).to_string();
        let e = ReadFailed::from(Error::NotARepository("/x".into()));
        assert_eq!(e, ReadFailed::NotARepository { path: "/x".into() });
        assert_eq!(e.to_string(), shown);
        let root = crate::paths::temp_dir().join("theirs");
        let refusal = || Refusal { root: root.clone(), owner: None, admins: false };
        let shown = Error::NotOwned(refusal()).to_string();
        let e = ReadFailed::from(Error::NotOwned(refusal()));
        assert_eq!(e, ReadFailed::NotOwned { root: crate::native_path::shown(&root) });
        assert_eq!(e.to_string(), shown);
    }

    #[test]
    fn open_failed_and_panicked_display_as_before() {
        let said = "could not find repository at 'r'; class=Repository (6)";
        let e = ReadFailed::OpenFailed { path: "r".into(), why: Box::new(ReadFailed::Said { said: said.into() }) };
        assert_eq!(e.to_string(), format!("Could not open r: {said}"));
        let e = ReadFailed::Panicked { effect: "load-repo".into(), panic: "boom".into() };
        assert_eq!(e.to_string(), "Glowfly hit an internal error (load-repo): boom");
    }

    #[test]
    fn every_mapping_keeps_the_errors_english() {
        let root = crate::paths::temp_dir().join("theirs");
        let missing = MissingContent { oids: vec![], remote: "origin".into(), path: "big.bin".into() };
        for e in [
            Error::Git(git2::Error::from_str("object not found")),
            Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone")),
            Error::NotARepository("/x".into()),
            Error::NotOwned(Refusal { root, owner: Some("someone".into()), admins: false }),
            Error::NotFound("no changes in a.txt".into()),
            Error::Unsupported("is a folder".into()),
            Error::Other("could not parse the merged file".into()),
            Error::MissingContent(missing),
        ] {
            let shown = e.to_string();
            assert_eq!(ReadFailed::from(e).to_string(), shown);
        }
    }

    #[test]
    fn glowfly_causes_display_as_their_old_error_text() {
        let p = "src/a.rs";
        let cases = [
            (ReadFailed::DiffChanged, Error::NotFound("the diff changed; try again on the updated diff".into())),
            (ReadFailed::FileChanged, Error::NotFound("the file changed".into())),
            (ReadFailed::OldFileUnreadable, Error::Unsupported("the old file can't be read".into())),
            (ReadFailed::NotConflicted { path: p.into() }, Error::NotFound(format!("{p} is not conflicted"))),
            (ReadFailed::NoFolder, Error::Unsupported("A folder history needs a folder".into())),
        ];
        for (typed, old) in cases {
            assert_eq!(typed.to_string(), old.to_string(), "{typed:?}");
        }
        assert_eq!(ReadFailed::NoThread { said: "out of memory".into() }.to_string(), "could not start a thread: out of memory");
        assert_eq!(ReadFailed::Internal { effect: "index".into() }.to_string(), "Glowfly hit an internal error (index)");
        assert_eq!(ReadFailed::Stopped.to_string(), "stopped");
        assert_eq!(ReadFailed::Said { said: "x; class=Os (2)".into() }.to_string(), "x; class=Os (2)");
    }
}
