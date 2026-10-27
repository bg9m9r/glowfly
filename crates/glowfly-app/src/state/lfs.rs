//! The Git LFS preferences page: the status read in the background, the new-pattern buffer and
//! what to do once the status arrives; the remote LFS commands use. Also whether git-lfs is
//! installed and the session's banner dismissals (`LfsSession`), the open diff's LFS summary
//! (`LfsDiffNote`, kept in `session::DiffSlot::lfs`; what they show is in `lfs_notes`), and the
//! Track in Git LFS entries of file rows' context menus.

use super::{AppState, branches};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::LfsChange;
use glowfly_core::lfs::{self, LfsPattern, LfsStatus, TrackTarget};
use glowfly_core::repo::{DiffTarget, Snapshot, SubmoduleInfo};
use std::path::PathBuf;

/// The job label of `git lfs install --local`; a successful one may offer to pull.
pub const INIT_LABEL: &str = "Initialize LFS";

/// Where git-lfs's installation help is (the page's link, the banner's Get Git LFS).
pub const INSTALL_URL: &str = "https://git-lfs.com";

/// Git LFS across the session: global. The open diff's note is `session::DiffSlot::lfs`, parked
/// with its tab.
#[derive(Default)]
pub struct LfsSession {
    /// Whether git-lfs is installed: checked at startup and whenever Preferences > Git LFS reads
    /// its status.
    pub install: LfsInstall,
    /// Workdirs whose "git-lfs missing" banner was dismissed, until Glowfly restarts.
    pub banner_dismissed: Vec<PathBuf>,
}

/// The open diff's LFS sides, shown as a summary instead of the pointer text.
#[derive(Clone, Debug, PartialEq)]
pub struct LfsDiffNote {
    pub target: DiffTarget,
    pub change: LfsChange,
    /// "Show pointer text": the normal rows instead of the summary. Kept across reloads of the
    /// same diff.
    pub show_pointer: bool,
}

#[derive(Default)]
pub struct LfsPage {
    /// None while the status is read, or when reading it failed (`error`).
    pub status: Option<LfsStatus>,
    pub error: Option<String>,
    /// Bumped by each read; an older read's result is ignored.
    pub generation: u64,
    /// The New pattern field (a view text buffer).
    pub new_pattern: String,
    /// Why the last Track was refused.
    pub pattern_error: Option<String>,
    /// The palette asked to initialize once the status allows it.
    pub init_when_ready: bool,
    /// A successful Initialize: offer to pull when the next status shows tracked patterns.
    pub offer_pull: bool,
    /// The last read was skipped: git is held in the repository. Trusting it reads again
    /// (update/lfs.rs `trust_released`).
    pub held: bool,
}

/// The remote LFS push goes to: HEAD's upstream remote, else origin, else the first remote.
pub fn lfs_remote(snap: &Snapshot) -> Option<String> {
    snap.head_info().and_then(branches::remote_tracking).map(|t| t.remote.clone()).or_else(|| branches::default_remote(snap))
}

/// Whether git-lfs is installed, for the file menu's Track in Git LFS entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LfsInstall {
    /// Not checked yet.
    #[default]
    Unknown,
    Checking,
    Missing,
    /// The version.
    Installed(String),
}

impl LfsInstall {
    /// From git-lfs's version, None when it isn't installed.
    pub fn from_version(v: Option<String>) -> Self {
        v.map_or(Self::Missing, Self::Installed)
    }
}

/// What a file row needs to build its Track in Git LFS entries.
#[derive(Clone, Copy)]
pub struct TrackContext<'a> {
    pub install: &'a LfsInstall,
    /// The root .gitattributes' LFS patterns.
    pub patterns: &'a [LfsPattern],
    pub submodules: &'a [SubmoduleInfo],
}

impl<'a> TrackContext<'a> {
    pub fn of(st: &'a AppState) -> Self {
        Self {
            install: &st.lfs.install,
            patterns: st.snap.as_ref().map_or(&[][..], |s| &s.lfs_patterns),
            submodules: st.snap.as_ref().map_or(&[][..], |s| &s.submodules),
        }
    }
}

/// One entry of the Track in Git LFS submenu.
pub struct TrackItem {
    pub label: String,
    /// What the entry writes to .gitattributes.
    pub hover: String,
    /// Why the entry is disabled.
    pub blocked: Option<String>,
    pub target: TrackTarget,
}

/// The Track in Git LFS entries for a file row's `path`: none for a submodule. Disabled without
/// git-lfs, and before the startup check has answered.
pub fn track_items(t: Lang, path: &str, cx: TrackContext) -> Vec<TrackItem> {
    if cx.submodules.iter().any(|s| s.path == path) {
        return Vec::new();
    }
    lfs::track_targets(path)
        .into_iter()
        .map(|target| {
            let shown: [(&str, &dyn std::fmt::Display); 1] = [("target", &target.display())];
            let (label, hover) = match &target {
                TrackTarget::File(_) => (t.tr("lfs.menu.only_file").to_owned(), t.trf("lfs.menu.adds.tip", &shown)),
                TrackTarget::Extension(_) => {
                    let ext = lfs::extension(path).unwrap_or_default();
                    (t.trf("lfs.menu.extension", &[("ext", &ext)]), t.trf("lfs.menu.adds.tip", &shown))
                }
                TrackTarget::Folder(_) => {
                    let folder = path.rsplit_once('/').map_or("", |(f, _)| f);
                    (t.trf("lfs.menu.folder", &[("folder", &folder)]), t.trf("lfs.menu.adds_folder.tip", &shown))
                }
            };
            let blocked = match cx.install {
                LfsInstall::Unknown | LfsInstall::Checking => Some(t.tr("lfs.menu.checking").to_owned()),
                LfsInstall::Missing => Some(t.tr("lfs.menu.not_installed").to_owned()),
                LfsInstall::Installed(_) => lfs::check_target(&target, cx.patterns).err().map(|p| p.message(t)),
            };
            TrackItem { hover, label, blocked, target }
        })
        .collect()
}
