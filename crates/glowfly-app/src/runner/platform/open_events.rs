//! Folders and links the OS hands the running process instead of putting them on the command
//! line: macOS sends them to Glowfly.app as Apple events (`platform/macos_events.rs`). They can arrive
//! before the window exists, so they wait here until the runner attaches, and then become the
//! start argument's messages (`StartArg::into_msg`). Elsewhere nothing posts here.

use crate::start_arg::StartArg;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Where items go once the runner is listening.
type Sink = Box<dyn Fn(StartArg) + Send>;

/// How long after startup an event naming the command line's own item is taken as the same
/// launch (macOS may hand over both) rather than a second request.
pub(crate) const START_REPEAT: Duration = Duration::from_secs(10);

/// The items that came before the runner, and where later ones go.
pub(crate) struct Inbox {
    /// The command line's item, not yet seen again as an event.
    start: Option<StartArg>,
    start_at: Option<Instant>,
    early: Vec<StartArg>,
    sink: Option<Sink>,
}

impl Inbox {
    pub(crate) const fn new() -> Self {
        Self { start: None, start_at: None, early: Vec::new(), sink: None }
    }

    /// The command line opened `start` at `now`: one event for the same item soon after is
    /// that launch's copy.
    pub(crate) fn expect_start(&mut self, start: Option<StartArg>, now: Instant) {
        self.start = start;
        self.start_at = Some(now);
    }

    /// An item the OS handed over at `now`: to the runner, or kept until it attaches.
    pub(crate) fn push(&mut self, item: StartArg, now: Instant) {
        let recent = self.start_at.is_some_and(|at| now.saturating_duration_since(at) <= START_REPEAT);
        if recent && self.start.as_ref().is_some_and(|s| same_item(s, &item)) {
            self.start = None;
            return;
        }
        match &self.sink {
            Some(sink) => sink(item),
            None => self.early.push(item),
        }
    }

    /// The runner listens from now on: the items kept so far go first, in order.
    pub(crate) fn attach(&mut self, sink: Sink) {
        for item in self.early.drain(..) {
            sink(item);
        }
        self.sink = Some(sink);
    }
}

/// Whether two items open the same thing: the same link text, or the same path as spelled (a
/// folder's file URL ends in a separator a typed path may lack).
fn same_item(a: &StartArg, b: &StartArg) -> bool {
    match (a, b) {
        (StartArg::Link(a), StartArg::Link(b)) => a == b,
        (StartArg::Path(a), StartArg::Path(b)) => glowfly_core::paths::same_spelling(a, b),
        _ => false,
    }
}

/// A link the OS handed over.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn link_item(text: String) -> StartArg {
    StartArg::Link(text)
}

/// A folder (or file) the OS handed over as a `file:` URL; None for any other URL.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn folder_item(url: &str) -> Option<StartArg> {
    glowfly_core::paths::from_file_url(url).map(StartArg::Path)
}

/// The Apple event codes Glowfly.app answers: four characters read as a big-endian number.
pub(crate) mod codes {
    #![cfg_attr(not(target_os = "macos"), allow(dead_code))]

    pub const fn four_cc(c: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*c)
    }

    /// A URL to open (`glowfly://...`): the internet event class and its get-URL event.
    pub const INTERNET_EVENT_CLASS: u32 = four_cc(b"GURL");
    pub const GET_URL: u32 = four_cc(b"GURL");
    /// Files and folders to open: the core event class and its open-documents event.
    pub const CORE_EVENT_CLASS: u32 = four_cc(b"aevt");
    pub const OPEN_DOCUMENTS: u32 = four_cc(b"odoc");
    /// The parameter holding what the event is about.
    pub const DIRECT_OBJECT: u32 = four_cc(b"----");
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox::new());

/// The process's inbox. Never panics: the macOS caller is an Objective-C callback.
fn inbox() -> MutexGuard<'static, Inbox> {
    INBOX.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts keeping the OS's items, before the window exists (`platform::install_open_events`);
/// `start` is what the command line already opens.
pub fn install(start: Option<&StartArg>) {
    inbox().expect_start(start.cloned(), Instant::now());
}

/// Sends every item, the early ones first, to `sink`. The sink runs with the inbox locked, so
/// it must not post here; the runner's only sends on its channel and wakes the window.
pub fn attach(sink: Sink) {
    inbox().attach(sink);
}

/// An item the OS handed over, from its event handler.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn received(item: StartArg) {
    match &item {
        StartArg::Link(text) => tracing::debug!("the OS handed over the link {}", crate::state::deep_link::shown_link(text)),
        StartArg::Path(p) => tracing::debug!("the OS handed over {}", glowfly_core::native_path::shown(p)),
    }
    inbox().push(item, Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc;

    fn link(s: &str) -> StartArg {
        StartArg::Link(s.to_owned())
    }

    fn path(name: &str) -> StartArg {
        StartArg::Path(glowfly_core::paths::temp_dir().join(name))
    }

    /// An inbox whose runner sends into the returned channel.
    fn attached(inbox: &mut Inbox) -> mpsc::Receiver<StartArg> {
        let (tx, rx) = mpsc::channel();
        inbox.attach(Box::new(move |item| {
            let _ = tx.send(item);
        }));
        rx
    }

    #[test]
    fn items_before_the_runner_wait_and_arrive_in_order() {
        let (mut inbox, t0) = (Inbox::new(), Instant::now());
        inbox.push(link("glowfly://repo?root=a"), t0);
        inbox.push(path("glowfly-open-b"), t0);
        let rx = attached(&mut inbox);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [link("glowfly://repo?root=a"), path("glowfly-open-b")]);
        inbox.push(path("glowfly-open-c"), t0);
        assert_eq!(rx.try_recv().ok(), Some(path("glowfly-open-c")), "delivered at once");
    }

    #[test]
    fn the_start_argument_is_not_opened_twice() {
        let t0 = Instant::now();
        let soon = t0 + Duration::from_secs(1);
        let repo = glowfly_core::paths::temp_dir().join("glowfly-open-repo");
        let mut slash = repo.clone().into_os_string();
        slash.push(std::path::MAIN_SEPARATOR_STR);
        for (start, again) in [
            (StartArg::Path(repo.clone()), StartArg::Path(PathBuf::from(slash))),
            (link("glowfly://tag/v1?url=x"), link("glowfly://tag/v1?url=x")),
        ] {
            let mut inbox = Inbox::new();
            inbox.expect_start(Some(start.clone()), t0);
            let rx = attached(&mut inbox);
            inbox.push(path("glowfly-open-other"), soon);
            inbox.push(again.clone(), soon);
            assert_eq!(rx.try_iter().collect::<Vec<_>>(), [path("glowfly-open-other")], "the launch's copy is dropped");
            inbox.push(again.clone(), soon);
            assert_eq!(rx.try_iter().collect::<Vec<_>>(), [again], "only once");
        }
    }

    #[test]
    fn a_repeat_after_startup_is_opened() {
        let t0 = Instant::now();
        let mut inbox = Inbox::new();
        inbox.expect_start(Some(path("glowfly-open-late")), t0);
        let rx = attached(&mut inbox);
        inbox.push(path("glowfly-open-late"), t0 + START_REPEAT + Duration::from_secs(1));
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [path("glowfly-open-late")]);
    }

    #[test]
    fn apple_event_codes_read_big_endian() {
        use codes::*;
        assert_eq!(CORE_EVENT_CLASS, 0x6165_7674);
        assert_eq!(OPEN_DOCUMENTS, 0x6F64_6F63);
        assert_eq!((INTERNET_EVENT_CLASS, GET_URL), (0x4755_524C, 0x4755_524C));
        assert_eq!(DIRECT_OBJECT, 0x2D2D_2D2D);
    }

    #[test]
    fn event_items_become_start_arguments() {
        assert_eq!(link_item("glowfly://tag/v1?url=x".into()), link("glowfly://tag/v1?url=x"));
        assert_eq!(folder_item("file:///w/My%20Repo/"), Some(StartArg::Path(PathBuf::from("/w/My Repo"))));
        assert_eq!(folder_item("https://x/"), None);
    }
}
