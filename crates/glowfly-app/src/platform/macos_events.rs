//! The Apple events macOS sends Glowfly.app instead of a command line: Get URL (`GURL`) for a
//! `glowfly://` link opened anywhere, and Open Documents (`odoc`) for a folder chosen under
//! Finder's Open With, dropped on the Dock icon or passed to `open -a`. packaging/macos/
//! Info.plist declares both, and each item goes to `open_events`, which holds it until the
//! runner listens.
//!
//! The handlers are installed twice. Once before eframe starts, which is enough for Get URL,
//! since AppKit has no handler of its own for it; and again when the app will finish launching,
//! because AppKit installs its own Open Documents handler while it launches and dispatches the
//! event that started the app right after that notification. Installing again replaces the
//! handler, so twice is harmless.

use super::open_events::{self, codes::*};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter};

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and OpenEvents doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "GlowflyOpenEvents"]
    struct OpenEvents;

    impl OpenEvents {
        #[unsafe(method(handleGetURL:withReplyEvent:))]
        fn handle_get_url(&self, event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            if let Some(text) = direct_object(event).and_then(|d| d.stringValue()) {
                open_events::received(open_events::link_item(text.to_string()));
            }
        }

        #[unsafe(method(handleOpenDocuments:withReplyEvent:))]
        fn handle_open_documents(&self, event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            let Some(list) = direct_object(event) else { return };
            let count = list.numberOfItems();
            // One item may come as itself rather than as a list of one.
            let items: Vec<Retained<NSAppleEventDescriptor>> =
                if count == 0 { vec![list] } else { (1..=count).filter_map(|i| list.descriptorAtIndex(i)).collect() };
            for item in items {
                let Some(url) = item.fileURLValue() else { continue };
                let url = if url.isFileReferenceURL() { url.filePathURL() } else { Some(url) };
                let Some(text) = url.and_then(|u| u.absoluteString()) else { continue };
                match open_events::folder_item(&text.to_string()) {
                    Some(it) => open_events::received(it),
                    None => tracing::warn!("macOS asked to open {text}, which isn't a local file"),
                }
            }
        }

        #[unsafe(method(applicationWillFinishLaunching:))]
        fn will_finish_launching(&self, _note: &NSNotification) {
            self.install_handlers();
        }
    }

    unsafe impl NSObjectProtocol for OpenEvents {}
);

impl OpenEvents {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init, which takes nothing and returns the object.
        unsafe { msg_send![super(this), init] }
    }

    /// Makes this object the handler of Get URL and Open Documents.
    fn install_handlers(&self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        for (class, id, selector) in [
            (INTERNET_EVENT_CLASS, GET_URL, sel!(handleGetURL:withReplyEvent:)),
            (CORE_EVENT_CLASS, OPEN_DOCUMENTS, sel!(handleOpenDocuments:withReplyEvent:)),
        ] {
            // SAFETY: self lives for the whole process (`install` leaks it); it answers the
            // selector with the (event, reply) signature the manager calls; the codes are u32,
            // as AEEventClass and AEEventID are. Raw because the typed method needs the
            // core-services bindings.
            let () = unsafe { msg_send![&*manager, setEventHandler: self, andSelector: selector, forEventClass: class, andEventID: id] };
        }
    }
}

/// The event's direct object: the URL text, or the list of items to open.
fn direct_object(event: &NSAppleEventDescriptor) -> Option<Retained<NSAppleEventDescriptor>> {
    // SAFETY: paramDescriptorForKeyword: takes an AEKeyword (u32) and returns a descriptor or
    // nil. Raw because the typed method needs the core-services bindings.
    unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] }
}

/// Installs the handlers now and again when the app will finish launching (see the module doc).
pub(super) fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("not on the main thread: macOS won't hand Glowfly links or folders");
        return;
    };
    let handler = OpenEvents::new(mtm);
    handler.install_handlers();
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: the handler answers the selector with one NSNotification, the extern static is
    // AppKit's constant for the notification, and the handler lives for the whole process.
    unsafe {
        center.addObserver_selector_name_object(
            &handler,
            sel!(applicationWillFinishLaunching:),
            Some(NSApplicationWillFinishLaunchingNotification),
            None,
        );
    }
    // Neither the event manager nor the notification centre retains it.
    std::mem::forget(handler);
}
