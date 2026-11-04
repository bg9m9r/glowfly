//! Why a check, a download or Install and restart failed, as the update results carry it
//! (#367): which step and what it was about, with the system's (or the hosting service's) own
//! words, rather than English text. The model words it in the user's language
//! (`state/error_words/typed.rs`); `Display` gives the English the headless
//! `glowfly --check-update` prints. Paths are kept as they are shown (`native_path::shown`).

use crate::forge::ServiceError;

/// Why a step of Glowfly's own update failed. `said` is the system's own words.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateError {
    /// GLOWFLY_UPDATE_FEED names a relative folder (`value`, printable).
    FeedNotAbsolute { value: String },
    /// The feed folder GLOWFLY_UPDATE_FEED names isn't there.
    FeedFolderMissing { dir: String },
    /// The hosting service refused, or couldn't be reached.
    Service(ServiceError),
    /// A file (the feed's release, the downloaded file) couldn't be read.
    ReadFailed { path: String, said: String },
    /// A file is longer than Glowfly reads (`name` is its file name).
    ReadTooLong { path: String, name: String },
    /// The running program's path couldn't be found.
    NoOwnProgram { said: String },
    /// The release has no file for this system.
    NoDownload,
    /// There is no Downloads (or home) folder to download into.
    NoDownloadsFolder,
    /// The release publishes no checksum for `name`.
    NoChecksum { name: String },
    /// The checksums couldn't be read, for `why`.
    Checksums(Box<UpdateError>),
    /// `name` is announced larger than Glowfly downloads.
    TooLarge { name: String, mb: u64 },
    /// The Downloads folder couldn't be created.
    CreateFailed { path: String, said: String },
    /// `name` couldn't be downloaded, for `why`.
    DownloadFailed { name: String, why: Box<UpdateError> },
    /// `name` is larger than it was announced.
    LargerThanExpected { name: String },
    /// A feed folder's file name Glowfly doesn't read.
    BadFileName { name: String },
    /// The download's SHA-256 isn't the published one.
    ChecksumMismatch,
    /// The download couldn't be written to disk.
    SaveFailed { path: String, said: String },
    /// The download ended at `got` bytes rather than the announced `want`.
    WrongSize { name: String, got: u64, want: u64 },
    /// The download was stopped before it finished (the headless check).
    Stopped,
    /// The downloaded file no longer matches the checksum it was downloaded with.
    ChangedSinceChecked,
    /// The installer couldn't be opened; `install_with` is the package tool (apt, dnf) a Linux
    /// package is installed with by hand.
    OpenFailed { path: String, said: String, install_with: Option<String> },
    /// This kind of update isn't installed by Install and restart.
    NotInPlace,
    /// An AppImage update, but this copy doesn't run from an AppImage.
    NotRunningFromAppImage,
    /// The AppImage update isn't an AppImage.
    NotAnAppImage,
    /// The portable copy's archive couldn't be read.
    ArchiveUnreadable { file: String, said: String },
    /// The download is neither a zip nor a tar.gz file.
    NotAnArchive { file: String },
    /// The archive unpacks to more than `mb` MB.
    ArchiveTooLarge { mb: u64 },
    /// The archive holds `name` twice.
    TwoCopies { name: String },
    /// `name` in the archive is a link or another special file.
    NotRegular { name: String },
    /// The archive has no `program`.
    NoProgram { program: String },
    /// A file couldn't be written (an MSI's copy, the helper's plan).
    WriteFailed { path: String, said: String },
    /// A file or folder couldn't be removed (undoing a preparation).
    RemoveFailed { path: String, said: String },
    /// No place to stage the helper worked: each place tried, with why.
    NoHelperPlace { tried: Vec<(String, String)> },
    /// The program in `folder` couldn't be replaced; `left` are the old files that couldn't be
    /// put back.
    ReplaceFailed { folder: String, said: String, left: Vec<String> },
    /// Putting the old files back failed: each file, with why.
    RollbackFailed { failed: Vec<(String, String)> },
    /// `error`, and then undoing what was done failed too (`undo`).
    NotUndone { error: Box<UpdateError>, undo: Box<UpdateError> },
    /// The update helper couldn't be started.
    HelperNotStarted { said: String },
    /// The update helper started without its input.
    HelperNoInput,
    /// The step panicked inside `effect` (the runner's guard).
    Internal { effect: String, panic: String },
    /// Anything else, in its own words.
    Said { said: String },
}

impl UpdateError {
    /// `self`, then the failure to undo what was done (`undo`), when there was one.
    pub fn and_undo(self, undo: Result<(), UpdateError>) -> Self {
        match undo {
            Ok(()) => self,
            Err(undo) => Self::NotUndone { error: Box::new(self), undo: Box::new(undo) },
        }
    }

    /// Each place with its reason, as "place: why; place: why".
    pub fn reasons(list: &[(String, String)]) -> String {
        list.iter().map(|(at, why)| format!("{at}: {why}")).collect::<Vec<_>>().join("; ")
    }
}

/// The English, as these errors read before they were typed.
impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeedNotAbsolute { value } => write!(f, "{} must name an absolute folder (got \"{value}\")", super::FEED_ENV),
            Self::FeedFolderMissing { dir } => write!(f, "The update feed folder {dir} doesn't exist"),
            Self::Service(e) => write!(f, "{e}"),
            Self::ReadFailed { path, said } => write!(f, "Couldn't read {path}: {said}"),
            Self::ReadTooLong { path, name } => write!(f, "Couldn't read {path}: {name} is larger than expected"),
            Self::NoOwnProgram { said } => write!(f, "Couldn't find Glowfly's own program: {said}"),
            Self::NoDownload => f.write_str("This release has no download for this system."),
            Self::NoDownloadsFolder => f.write_str("No folder to download into"),
            Self::NoChecksum { name } => write!(f, "This release doesn't publish a checksum for {name}, so it wasn't downloaded."),
            Self::Checksums(why) => write!(f, "Couldn't read the checksums: {why}"),
            Self::TooLarge { name, mb } => write!(f, "{name} is larger than {mb} MB, so it wasn't downloaded."),
            Self::CreateFailed { path, said } => write!(f, "Couldn't create {path}: {said}"),
            Self::DownloadFailed { name, why } => write!(f, "Couldn't download {name}: {why}"),
            Self::LargerThanExpected { name } => write!(f, "{name} is larger than expected"),
            Self::BadFileName { name } => write!(f, "{name} isn't a file name Glowfly reads"),
            Self::ChecksumMismatch => f.write_str("The download didn't match its published checksum and was discarded."),
            Self::SaveFailed { path, said } => write!(f, "Couldn't save {path}: {said}"),
            Self::WrongSize { name, got, want } => {
                write!(f, "The download of {name} was {got} bytes instead of {want}, so it was discarded.")
            }
            Self::Stopped => f.write_str("The download was stopped."),
            Self::ChangedSinceChecked => {
                f.write_str("The downloaded file changed after it was checked, so it wasn't installed. Download it again.")
            }
            Self::OpenFailed { path, said, install_with } => {
                write!(f, "Couldn't open {path}: {said}")?;
                match install_with {
                    Some(tool) => write!(f, "\nInstall it with: sudo {tool} install {path}"),
                    None => Ok(()),
                }
            }
            Self::NotInPlace => f.write_str("This update can't be installed in place."),
            Self::NotRunningFromAppImage => f.write_str("This copy isn't running from an AppImage."),
            Self::NotAnAppImage => f.write_str("The download isn't an AppImage."),
            Self::ArchiveUnreadable { file, said } => write!(f, "Couldn't read the archive {file}: {said}"),
            Self::NotAnArchive { file } => write!(f, "Couldn't read the archive {file}: it isn't a zip or tar.gz file"),
            Self::ArchiveTooLarge { mb } => write!(f, "The archive is larger than {mb} MB when unpacked."),
            Self::TwoCopies { name } => write!(f, "The archive has two copies of {name}."),
            Self::NotRegular { name } => write!(f, "{name} in the archive isn't a regular file."),
            Self::NoProgram { program } => write!(f, "The archive has no {program}."),
            Self::WriteFailed { path, said } => write!(f, "Couldn't write {path}: {said}"),
            Self::RemoveFailed { path, said } => write!(f, "Couldn't remove {path}: {said}"),
            Self::NoHelperPlace { tried } => write!(f, "Couldn't find a folder the update helper can run from ({}).", Self::reasons(tried)),
            Self::ReplaceFailed { folder, said, left } => {
                write!(f, "Couldn't replace Glowfly's program in {folder}: {said}. Use Show file to install the update by hand.")?;
                left.iter().try_for_each(|b| write!(f, " The old file is at {b}."))
            }
            Self::RollbackFailed { failed } => f.write_str(&Self::reasons(failed)),
            Self::NotUndone { error, undo } => write!(f, "{error} {undo}"),
            Self::HelperNotStarted { said } => write!(f, "Couldn't start the update helper: {said}"),
            Self::HelperNoInput => f.write_str("Couldn't start the update helper: it has no input"),
            Self::Internal { effect, panic } => write!(f, "Glowfly hit an internal error ({effect}): {panic}"),
            Self::Said { said } => f.write_str(said),
        }
    }
}
