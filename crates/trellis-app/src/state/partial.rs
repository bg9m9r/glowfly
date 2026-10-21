//! A shallow clone's size check before Fetch full history, and a partial clone's missing file
//! contents: which view waits for them, the download running for it, and what the waiting
//! pane offers (update/partial_size.rs, update/partial_content.rs, ui/content_fetch.rs).

use super::AppState;
use crate::effect::Effect;
use git2::Oid;
use std::collections::HashSet;
use std::path::PathBuf;

/// The label of the job that downloads missing contents from a waiting pane's button.
pub const DOWNLOAD_LABEL: &str = "Download file contents";

/// The active tab's size check and missing contents (forgotten on a tab switch).
#[derive(Default)]
pub struct PartialState {
    /// The last id handed out, for size checks and downloads alike.
    pub next_id: u64,
    /// The size check running for Fetch full history.
    pub sizing: Option<Sizing>,
    /// The view waiting for contents, and its download.
    pub content: Option<PendingContent>,
    /// Ids a download that succeeded asked for: a read still missing one of them doesn't
    /// download again.
    pub fetched: HashSet<Oid>,
}

impl PartialState {
    /// A new id for a size check or a download.
    pub fn take_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

/// A running size check.
#[derive(Clone, Debug, PartialEq)]
pub struct Sizing {
    pub id: u64,
    pub workdir: PathBuf,
    /// The remote the full history would come from.
    pub remote: String,
}

/// A view whose read waits for missing contents.
#[derive(Debug)]
pub struct PendingContent {
    pub workdir: PathBuf,
    pub view: ContentView,
    pub path: String,
    pub remote: String,
    pub oids: Vec<Oid>,
    /// The read to run again once the contents are downloaded.
    pub retry: Option<Box<Effect>>,
    pub phase: ContentPhase,
    /// Its download job waits for the trust prompt.
    pub held: bool,
}

/// Which view a read was for, and the request that tells it is still the one on screen.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentView {
    /// The main diff, the file history's Diff pane or the pull request panel's diff.
    Diff {
        generation: u64,
    },
    /// The file history's File view.
    FileText {
        oid: Oid,
        path: String,
        before: bool,
    },
    Blame {
        generation: u64,
    },
}

impl ContentView {
    /// The view `retry` reads for; None for an effect that isn't one of those reads.
    pub fn of(retry: &Effect) -> Option<ContentView> {
        match retry {
            Effect::LoadDiff { generation, .. } => Some(ContentView::Diff { generation: *generation }),
            Effect::LoadFileText(r) => Some(ContentView::FileText { oid: r.oid, path: r.path.clone(), before: r.before }),
            Effect::LoadBlame(r) => Some(ContentView::Blame { generation: r.generation }),
            _ => None,
        }
    }

    /// The same kind of view (a newer read of it).
    pub fn same_kind(&self, other: &ContentView) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Where the download stands.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentPhase {
    /// Downloading in the background; `stopping` once Cancel was pressed.
    Fetching {
        id: u64,
        stopping: bool,
    },
    /// Downloading as the active job (`DOWNLOAD_LABEL`).
    FetchingAsJob,
    Waiting(Wait),
}

/// Why nothing is downloading.
#[derive(Clone, Debug, PartialEq)]
pub enum Wait {
    /// The repository isn't trusted: downloading runs git there, so it asks first.
    Trust,
    /// The download failed (git's last line).
    Failed(String),
    Cancelled,
    /// A download that succeeded didn't bring them.
    StillMissing,
    /// git is too old (the reason).
    OldGit(String),
}

impl AppState {
    /// The waiting contents of the diff being opened, while it is the latest diff request.
    pub fn for_diff(&self) -> Option<&PendingContent> {
        self.partial.content.as_ref().filter(|p| p.view == ContentView::Diff { generation: self.diff_gen })
    }

    /// The waiting contents of the File view's text `oid`/`path`/`before`.
    pub fn for_file_text(&self, oid: Oid, path: &str, before: bool) -> Option<&PendingContent> {
        let view = ContentView::FileText { oid, path: path.to_owned(), before };
        self.partial.content.as_ref().filter(|p| p.view == view)
    }

    /// The waiting contents of the blame being loaded.
    pub fn for_blame(&self) -> Option<&PendingContent> {
        let loading = self.blame.as_ref().is_some_and(|b| b.loading);
        self.partial.content.as_ref().filter(|p| loading && p.view == ContentView::Blame { generation: self.blame_gen })
    }

    /// A diff is shown, or the diff being opened waits for its contents (the central area shows
    /// the diff's place either way).
    pub fn diff_or_download(&self) -> bool {
        self.diff.is_some() || self.for_diff().is_some()
    }

    /// `p`'s phase as its pane shows it: a download job that isn't running (dropped before it
    /// started) offers the download again.
    pub fn content_phase(&self, p: &PendingContent) -> ContentPhase {
        let running = self.job.as_ref().is_some_and(|j| j.label == DOWNLOAD_LABEL);
        match &p.phase {
            ContentPhase::FetchingAsJob if !running => ContentPhase::Waiting(Wait::Cancelled),
            phase => phase.clone(),
        }
    }

    /// Whether `view` is still what its view on screen asked for.
    pub fn content_view_current(&self, view: &ContentView) -> bool {
        match view {
            ContentView::Diff { generation } => *generation == self.diff_gen,
            ContentView::FileText { oid, path, before } => self.history.as_ref().and_then(|h| h.text.as_ref()).is_some_and(|t| {
                t.oid == *oid && t.path == *path && t.before == *before && matches!(t.state, super::history::TextState::Loading)
            }),
            ContentView::Blame { generation } => *generation == self.blame_gen && self.blame.as_ref().is_some_and(|b| b.loading),
        }
    }
}
