//! The AppKit side of the macOS system menu bar: builds the `NSMenu`s of a plan
//! (`state/menu_native.rs`) and updates them in place. Every item of Glowfly's sends `chosen:`
//! to one target, which queues the item's tag (`native_menu::picked`). The same object is every
//! menu's delegate and tells AppKit that no menu handles a key press, so every key reaches the
//! window and the keymap; only Cmd+H and Option+Cmd+H go to AppKit's Hide and Hide Others.

use super::native_menu;
use crate::state::menu_native::{KeyEquivalent, NEntry, Plan, Standard, standard_key};
use objc2::encode::{Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSMenu, NSMenuDelegate, NSMenuItem,
};
use objc2_foundation::NSString;

/// Where AppKit's key-equivalent question takes the action (`SEL *`).
#[repr(transparent)]
struct SelSlot(Option<Sel>);

// SAFETY: SelSlot is a transparent `Option<Sel>`, which has the layout of a nullable `SEL`, so a
// pointer to it is a `SEL *`.
unsafe impl RefEncode for SelSlot {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Sel);
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and MenuTarget doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "GlowflyMenuTarget"]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(chosen:))]
        fn chosen(&self, item: &NSMenuItem) {
            native_menu::picked(i64::try_from(item.tag()).unwrap_or(-1));
        }

        #[unsafe(method(menuHasKeyEquivalent:forEvent:target:action:))]
        fn has_key_equivalent(&self, _menu: &NSMenu, event: &NSEvent, target: *mut *mut AnyObject, action: *mut SelSlot) -> bool {
            match appkit_key(event) {
                Some(sel) if !target.is_null() && !action.is_null() => {
                    let app = NSApplication::sharedApplication(self.mtm());
                    // SAFETY: AppKit passes pointers to its own variables, valid for this call,
                    // and reads the target unretained; NSApp lives for the whole process.
                    unsafe {
                        *target = Retained::as_ptr(&app).cast_mut().cast();
                        *action = SelSlot(Some(sel));
                    }
                    true
                }
                // Glowfly's keys go to the window.
                _ => false,
            }
        }
    }

    unsafe impl NSObjectProtocol for MenuTarget {}

    unsafe impl NSMenuDelegate for MenuTarget {}
);

/// The selector of the app menu item AppKit answers for `event`'s key (`standard_key`).
fn appkit_key(event: &NSEvent) -> Option<Sel> {
    let chars = event.charactersIgnoringModifiers().map(|s| s.to_string()).unwrap_or_default();
    let f = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    let has = |m: NSEventModifierFlags| f.contains(m);
    let (command, shift) = (has(NSEventModifierFlags::Command), has(NSEventModifierFlags::Shift));
    let (alt, ctrl) = (has(NSEventModifierFlags::Option), has(NSEventModifierFlags::Control));
    standard_key(&chars, command, shift, alt, ctrl).map(standard_sel)
}

/// An item as built, with its submenu and its rows (to update it in place).
struct Node {
    item: Retained<NSMenuItem>,
    sub: Option<Retained<NSMenu>>,
    rows: Vec<Node>,
}

/// The installed menu bar.
pub struct Bar {
    /// Every item's target and every menu's delegate, both weak in AppKit: kept here.
    target: Retained<MenuTarget>,
    /// The app menu, then File … Help.
    tops: Vec<Node>,
}

impl Bar {
    /// The target, on the main thread only (else a warning and None).
    pub fn install() -> Option<Bar> {
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::warn!("not on the main thread: the menu bar keeps AppKit's menus");
            return None;
        };
        let this = MenuTarget::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init, which takes nothing and returns the object.
        let target: Retained<MenuTarget> = unsafe { msg_send![super(this), init] };
        Some(Bar { target, tops: Vec::new() })
    }

    /// Replaces the menu bar with `plan`'s menus.
    pub fn rebuild(&mut self, plan: &Plan) {
        let mtm = self.target.mtm();
        let app = NSApplication::sharedApplication(mtm);
        let bar = self.menu("");
        // AppKit shows the bundle's name as the app menu's title.
        let tops: Vec<(&str, &[NEntry])> = std::iter::once(("Glowfly", plan.app.as_slice()))
            .chain(plan.menus.iter().map(|m| (m.title.as_str(), m.entries.as_slice())))
            .collect();
        self.tops = tops.into_iter().map(|(title, rows)| self.submenu(&app, title, rows)).collect();
        for top in &self.tops {
            bar.addItem(&top.item);
        }
        app.setMainMenu(Some(&bar));
        app.setHelpMenu(self.tops.last().and_then(|t| t.sub.as_deref()));
    }

    /// Updates the menus built from `old` to show `new`, which has the same shape.
    pub fn update(&self, old: &Plan, new: &Plan) {
        let olds = std::iter::once(&old.app).chain(old.menus.iter().map(|m| &m.entries));
        let news = std::iter::once(&new.app).chain(new.menus.iter().map(|m| &m.entries));
        for ((top, o), n) in self.tops.iter().zip(olds).zip(news) {
            update_rows(&top.rows, o, n);
        }
        for (top, (o, n)) in self.tops.iter().skip(1).zip(old.menus.iter().zip(&new.menus)) {
            if o.title != n.title {
                set_title(top, &n.title);
            }
        }
    }

    /// A new menu titled `title` that answers to the target.
    fn menu(&self, title: &str) -> Retained<NSMenu> {
        let m = NSMenu::new(self.target.mtm());
        m.setTitle(&NSString::from_str(title));
        m.setAutoenablesItems(false);
        m.setDelegate(Some(ProtocolObject::from_ref(&*self.target)));
        m
    }

    /// An item titled `title` opening a menu of `rows`.
    fn submenu(&self, app: &NSApplication, title: &str, rows: &[NEntry]) -> Node {
        let sub = self.menu(title);
        let rows: Vec<Node> = rows.iter().map(|e| self.row(app, e)).collect();
        for r in &rows {
            sub.addItem(&r.item);
        }
        let item = new_item(self.target.mtm(), title, None, None);
        item.setSubmenu(Some(&sub));
        Node { item, sub: Some(sub), rows }
    }

    fn row(&self, app: &NSApplication, e: &NEntry) -> Node {
        let mtm = self.target.mtm();
        let leaf = |item| Node { item, sub: None, rows: Vec::new() };
        match e {
            NEntry::Item { tag, title, key, enabled, on, tip } => {
                let item = new_item(mtm, title, Some(sel!(chosen:)), key.as_ref());
                // SAFETY: `Bar` keeps the target retained for as long as the menus exist, and
                // the target answers `chosen:` with the item.
                unsafe { item.setTarget(Some(&self.target)) };
                item.setTag(isize::try_from(*tag).unwrap_or(-1));
                item.setEnabled(*enabled);
                item.setState(state(*on));
                set_tip(&item, tip.as_deref());
                leaf(item)
            }
            // A nil target: the responder chain ends at NSApp, which answers them.
            NEntry::Standard { title, key, action } => leaf(new_item(mtm, title, Some(standard_sel(*action)), key.as_ref())),
            NEntry::Services(title) => {
                let services = NSMenu::new(mtm);
                services.setTitle(&NSString::from_str(title));
                let item = new_item(mtm, title, None, None);
                item.setSubmenu(Some(&services));
                app.setServicesMenu(Some(&services));
                Node { item, sub: Some(services), rows: Vec::new() }
            }
            NEntry::Separator => leaf(NSMenuItem::separatorItem(mtm)),
            NEntry::Heading(title) => {
                let item = new_item(mtm, title, None, None);
                item.setEnabled(false);
                leaf(item)
            }
            NEntry::Submenu { title, enabled, tip, entries } => {
                let node = self.submenu(app, title, entries);
                node.item.setEnabled(*enabled);
                set_tip(&node.item, tip.as_deref());
                node
            }
        }
    }
}

/// Sets only what changed from `old` to `new` on the items built for `old`.
fn update_rows(nodes: &[Node], old: &[NEntry], new: &[NEntry]) {
    for ((node, o), n) in nodes.iter().zip(old).zip(new) {
        match (o, n) {
            (
                NEntry::Item { tag: t0, title: n0, key: k0, enabled: e0, on: o0, tip: p0 },
                NEntry::Item { tag, title, key, enabled, on, tip },
            ) => {
                if n0 != title {
                    set_title(node, title);
                }
                if k0 != key {
                    node.item.setKeyEquivalent(&NSString::from_str(key.as_ref().map_or("", |k| k.key.as_str())));
                    node.item.setKeyEquivalentModifierMask(mask(key.as_ref()));
                }
                if e0 != enabled {
                    node.item.setEnabled(*enabled);
                }
                if o0 != on {
                    node.item.setState(state(*on));
                }
                if p0 != tip {
                    set_tip(&node.item, tip.as_deref());
                }
                if t0 != tag {
                    node.item.setTag(isize::try_from(*tag).unwrap_or(-1));
                }
            }
            (NEntry::Submenu { title: n0, enabled: e0, tip: p0, entries: r0 }, NEntry::Submenu { title, enabled, tip, entries }) => {
                if n0 != title {
                    set_title(node, title);
                }
                if e0 != enabled {
                    node.item.setEnabled(*enabled);
                }
                if p0 != tip {
                    set_tip(&node.item, tip.as_deref());
                }
                update_rows(&node.rows, r0, entries);
            }
            (NEntry::Standard { title: n0, .. }, NEntry::Standard { title, .. })
            | (NEntry::Services(n0), NEntry::Services(title))
            | (NEntry::Heading(n0), NEntry::Heading(title))
                if n0 != title =>
            {
                set_title(node, title);
            }
            _ => {}
        }
    }
}

/// Titles the item and its submenu.
fn set_title(node: &Node, title: &str) {
    let title = NSString::from_str(title);
    node.item.setTitle(&title);
    if let Some(sub) = &node.sub {
        sub.setTitle(&title);
    }
}

fn set_tip(item: &NSMenuItem, tip: Option<&str>) {
    item.setToolTip(tip.map(NSString::from_str).as_deref());
}

fn state(on: bool) -> isize {
    if on { NSControlStateValueOn } else { NSControlStateValueOff }
}

/// AppKit's selector for an app menu item it answers.
fn standard_sel(s: Standard) -> Sel {
    match s {
        Standard::Hide => sel!(hide:),
        Standard::HideOthers => sel!(hideOtherApplications:),
        Standard::ShowAll => sel!(unhideAllApplications:),
    }
}

/// The modifier mask of `k`, set always: AppKit's default is Cmd.
fn mask(k: Option<&KeyEquivalent>) -> NSEventModifierFlags {
    let mut m = NSEventModifierFlags::empty();
    if let Some(k) = k {
        for (on, flag) in [
            (k.command, NSEventModifierFlags::Command),
            (k.shift, NSEventModifierFlags::Shift),
            (k.alt, NSEventModifierFlags::Option),
            (k.ctrl, NSEventModifierFlags::Control),
        ] {
            if on {
                m |= flag;
            }
        }
    }
    m
}

/// An item titled `title` that sends `action` and shows `key`.
fn new_item(mtm: MainThreadMarker, title: &str, action: Option<Sel>, key: Option<&KeyEquivalent>) -> Retained<NSMenuItem> {
    let chars = NSString::from_str(key.map_or("", |k| k.key.as_str()));
    // SAFETY: the selectors name methods with one sender argument that the target or the
    // responder chain answers.
    let item =
        unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), action, &chars) };
    item.setKeyEquivalentModifierMask(mask(key));
    item
}
