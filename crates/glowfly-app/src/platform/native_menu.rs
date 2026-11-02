//! The macOS system menu bar's facade: the shell (`ui/app_menu.rs`) hands it the menu model
//! after each frame's messages, and it updates the AppKit menus (`macos_menu.rs`) in place, or
//! builds them anew when their rows changed, with a new generation of tags. A click on an item
//! comes back as its tag through a queue (`picked`), which wakes the window; the next frame takes
//! the tags and turns those of the current generation into the model's actions. Elsewhere, and
//! with another frame, nothing is installed.

// Outside macOS only the facade's signatures are used.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::state::menu_model::{Menu, MenuAction};
use crate::state::menu_native::{self, Plan};
use crate::state::window_chrome::Chrome;
use eframe::egui;
use glowfly_core::i18n::Lang;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Wakes the window after a pick.
type Waker = Box<dyn Fn() + Send>;

/// The tags of the items clicked since the last frame took them.
#[derive(Default)]
struct Picks {
    tags: Vec<i64>,
    waker: Option<Waker>,
}

impl Picks {
    const fn new() -> Self {
        Self { tags: Vec::new(), waker: None }
    }

    fn push(&mut self, tag: i64) {
        self.tags.push(tag);
        if let Some(wake) = &self.waker {
            wake();
        }
    }

    fn take(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.tags)
    }

    fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }
}

static PICKS: Mutex<Picks> = Mutex::new(Picks::new());

/// The queue, even after a panic elsewhere (the callers are Objective-C callbacks).
fn picks() -> MutexGuard<'static, Picks> {
    PICKS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An item with `tag` was chosen (from the menu target's action).
pub(crate) fn picked(tag: i64) {
    picks().push(tag);
}

fn take() -> Vec<i64> {
    picks().take()
}

/// The system menu bar showing Glowfly's menus.
pub struct NativeMenu {
    #[cfg(target_os = "macos")]
    bar: super::macos_menu::Bar,
    /// The model the shown menus came from: picks take their actions from it.
    model: Option<[Menu; 5]>,
    shown: Option<Plan>,
    generation: u32,
}

impl NativeMenu {
    /// Turns the system menu bar over to Glowfly's menus: only on macOS (`Chrome::native_menu`);
    /// `ctx` is woken on every pick.
    pub fn install(chrome: Chrome, ctx: egui::Context) -> Option<NativeMenu> {
        #[cfg(target_os = "macos")]
        {
            if !chrome.native_menu() {
                return None;
            }
            let bar = super::macos_menu::Bar::install()?;
            picks().set_waker(Box::new(move || ctx.request_repaint()));
            Some(NativeMenu { bar, model: None, shown: None, generation: 0 })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (chrome, ctx);
            None
        }
    }

    /// The actions of the items chosen since the last call; clicks on menus from before a
    /// rebuild are dropped.
    pub fn take_picked(&mut self) -> Vec<MenuAction> {
        let tags = take();
        match &mut self.model {
            Some(model) => menu_native::take_picked(model, self.generation, &tags),
            None => Vec::new(),
        }
    }

    /// Shows `menus` in language `t`: in place when only titles, states and keys changed, else
    /// built anew under the next generation of tags.
    pub fn show(&mut self, menus: [Menu; 5], t: Lang) {
        let mut plan = menu_native::plan(&menus, t, self.generation);
        match &self.shown {
            Some(old) if menu_native::same_shape(old, &plan) => {
                if *old != plan {
                    #[cfg(target_os = "macos")]
                    self.bar.update(old, &plan);
                }
            }
            _ => {
                self.generation = self.generation.wrapping_add(1);
                plan = menu_native::plan(&menus, t, self.generation);
                #[cfg(target_os = "macos")]
                self.bar.rebuild(&plan);
            }
        }
        self.shown = Some(plan);
        self.model = Some(menus);
    }
}

/// Turns winit's default app menu off on macOS: its Quit would end the app through AppKit's
/// `terminate:`, without a close request (`NativeMenu` shows Glowfly's own).
#[cfg(target_os = "macos")]
pub fn event_loop_hook() -> eframe::EventLoopBuilderHook {
    use winit::platform::macos::EventLoopBuilderExtMacOS;
    Box::new(|b| {
        b.with_default_menu(false);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn picks_queue_in_order_and_wake() {
        let mut p = Picks::new();
        p.push(7);
        assert_eq!(p.take(), [7], "a pick with no waker set is kept");
        let woken = Arc::new(AtomicUsize::new(0));
        let w = woken.clone();
        p.set_waker(Box::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }));
        p.push(1);
        p.push(2);
        assert_eq!(woken.load(Ordering::SeqCst), 2, "once per pick");
        assert_eq!(p.take(), [1, 2]);
        assert!(p.take().is_empty());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn no_hook_and_no_menu_off_macos() {
        // Off macOS the event loop has no menu hook (`platform::event_loop_hook`: Windows has
        // only its keypad hook).
        #[cfg(not(windows))]
        assert!(crate::platform::event_loop_hook().is_none());
        assert!(NativeMenu::install(Chrome::MacOs, egui::Context::default()).is_none());
    }
}
