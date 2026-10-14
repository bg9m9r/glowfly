//! The egui layer. Views draw `AppState` and push `Msg`s; the shell feeds those messages and
//! background results through `update`, runs the resulting effects, and handles the few
//! effects that need the window.

mod askpass;
mod avatars;
mod branch_dialogs;
mod branch_menu;
pub(crate) mod common;
mod conflict;
mod details;
mod details_multi;
mod diff;
mod graph;
mod history;
mod menus;
mod modals;
mod prefs;
mod publish;
mod rebase;
mod ref_visibility;
mod sidebar;
mod spell_edit;
mod status;
mod tabs;
pub mod theme;
mod toolbar;
pub mod widgets;

#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod snapshot_tests_askpass;
#[cfg(test)]
mod snapshot_tests_branch;
#[cfg(test)]
mod snapshot_tests_diff;
#[cfg(test)]
mod snapshot_tests_editor;
#[cfg(test)]
mod snapshot_tests_rebase;
#[cfg(test)]
mod snapshot_tests_revert;
#[cfg(test)]
mod snapshot_tests_select;
#[cfg(test)]
mod snapshot_tests_whole;
#[cfg(test)]
mod tabs_tests;

use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::runner::Runner;
use crate::state::AppState;
use crate::update::update;
use eframe::egui::{self, Key};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use theme::color;
use trellis_core::settings::{Settings, ThemeChoice};
use trellis_core::spell::Speller;

/// Resources owned by the UI thread (textures, the spell checker's COM object).
pub struct UiCache {
    pub avatars: avatars::Avatars,
    pub speller: Speller,
}

pub struct TrellisApp {
    st: AppState,
    cache: UiCache,
    runner: Runner<egui::Context>,
    results: Receiver<Msg>,
}

impl TrellisApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        theme::apply_theme(&cc.egui_ctx);
        let (tx, results) = mpsc::channel();
        let settings = Settings::load();
        let recent = crate::platform::load_recent();
        // Restore the last session's tabs (or, before tabs were saved, the last repository).
        let (mut paths, active) = if settings.reopen_last_repo { crate::platform::load_tabs() } else { (Vec::new(), 0) };
        if paths.is_empty() && settings.reopen_last_repo {
            paths.extend(recent.first().cloned());
        }
        let mut app = Self {
            st: AppState::new(settings, recent, Instant::now()),
            cache: UiCache { avatars: avatars::Avatars::default(), speller: Speller::new() },
            runner: Runner::new(tx, cc.egui_ctx.clone()),
            results,
        };
        // git and ssh ask for missing credentials through a dialog from now on.
        app.runner.start_askpass();
        if !paths.is_empty() {
            app.dispatch(&cc.egui_ctx, Msg::RestoreTabs { paths, active });
        }
        if let Some(path) = start {
            app.dispatch(&cc.egui_ctx, Msg::Open(path));
        }
        app
    }

    fn dispatch(&mut self, ctx: &egui::Context, msg: Msg) {
        for effect in update(&mut self.st, msg, Instant::now()) {
            match effect {
                Effect::Ui(UiEffect::CopyText(text)) => ctx.copy_text(text),
                Effect::Ui(UiEffect::SetTitle(title)) => ctx.send_viewport_cmd(egui::ViewportCommand::Title(title)),
                Effect::Ui(UiEffect::RequestAttention) => {
                    if !ctx.input(|i| i.focused) {
                        let attention = egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational);
                        ctx.send_viewport_cmd(attention);
                    }
                }
                other => self.runner.run(other),
            }
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let dark = match self.st.settings.theme {
            ThemeChoice::Dark => true,
            ThemeChoice::Light => false,
            ThemeChoice::System => ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark),
        };
        color::set_dark(dark);
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    }

    /// Keyboard shortcuts and window events, turned into messages or view-state changes.
    fn input(&mut self, ctx: &egui::Context, msgs: &mut Vec<Msg>) {
        let focused = ctx.input(|i| i.focused);
        if focused && !self.st.view.was_focused {
            msgs.push(Msg::WindowFocused);
        }
        self.st.view.was_focused = focused;
        let (f5, find, prefs) = ctx.input(|i| {
            (i.key_pressed(Key::F5), i.modifiers.command && i.key_pressed(Key::F), i.modifiers.command && i.key_pressed(Key::Comma))
        });
        if self.st.prefs.is_none() {
            tab_shortcuts(ctx, self.st.tabs.len(), msgs);
        }
        // Text fields keep Ctrl+Z for their own undo; nothing reaches the views under a dialog.
        let dialog = self.st.modal.is_some() || self.st.askpass.showing();
        if self.st.prefs.is_none() && !dialog && !ctx.wants_keyboard_input() {
            undo_shortcuts(ctx, msgs);
        }
        // Text fields (the commit message) keep Alt+arrows.
        let stepping = self.st.conflict.is_some() || self.st.diff.is_some();
        if stepping
            && self.st.prefs.is_none()
            && !dialog
            && !ctx.wants_keyboard_input()
            && let Some(step) = step_shortcuts(ctx)
        {
            msgs.push(if self.st.conflict.is_some() { Msg::ConflictStep(step) } else { Msg::JumpToChange(step) });
        }
        if f5 {
            msgs.push(Msg::Refresh);
        }
        if find {
            self.st.view.show_search = !self.st.view.show_search;
        }
        if prefs && self.st.prefs.is_none() {
            msgs.push(Msg::OpenPrefs(crate::state::PrefPage::General));
        }
        escape_key(ctx, &mut self.st, msgs);
    }
}

/// Esc closes the innermost thing open: Preferences, the conflict view, the file history, the
/// diff (a line selection first), the interactive rebase panel (its message editor first), then
/// a multi-selection, and finally the search bar. Dialogs (a credential prompt included) handle
/// Esc themselves.
fn escape_key(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !ctx.input(|i| i.key_pressed(Key::Escape)) || st.modal.is_some() || st.askpass.showing() {
        return;
    }
    if st.prefs.is_some() {
        msgs.push(Msg::ClosePrefs);
    } else if st.conflict.is_some() {
        msgs.push(Msg::CloseConflict);
    } else if st.history.is_some() {
        msgs.push(Msg::CloseFileHistory);
    } else if st.diff.is_some() {
        // Esc first drops a line selection, then closes the diff.
        msgs.push(if st.view.diff_lines.is_empty() { Msg::CloseDiff } else { Msg::ClearLineSelection });
    } else if let Some(p) = &st.rebase {
        // Esc first closes the message editor, then the panel.
        msgs.push(if p.editing.is_some() { Msg::RebaseCancelMessage } else { Msg::CloseRebase });
    } else if matches!(st.selection, crate::state::Selection::Commits(_)) {
        msgs.push(Msg::CollapseSelection);
    } else {
        st.view.show_search = false;
    }
}

/// Ctrl+T new tab, Ctrl+W close, Ctrl+Shift+T reopen, Ctrl+(Shift+)Tab cycle, Ctrl+1–8 select,
/// Ctrl+9 last tab.
fn tab_shortcuts(ctx: &egui::Context, count: usize, msgs: &mut Vec<Msg>) {
    use egui::Modifiers;
    const DIGITS: [Key; 9] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    ctx.input_mut(|i| {
        let shift = Modifiers::COMMAND | Modifiers::SHIFT;
        if i.consume_key(shift, Key::T) {
            msgs.push(Msg::ReopenClosedTab);
        } else if i.consume_key(Modifiers::COMMAND, Key::T) {
            msgs.push(Msg::NewTab);
        }
        if i.consume_key(Modifiers::COMMAND, Key::W) {
            msgs.push(Msg::CloseTab(usize::MAX));
        }
        // Ctrl+Tab, matched exactly so it doesn't also count as Ctrl+Shift+Tab.
        if i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab) {
            msgs.push(Msg::CycleTab(-1));
        } else if i.consume_key(Modifiers::CTRL, Key::Tab) {
            msgs.push(Msg::CycleTab(1));
        }
        for (n, key) in DIGITS.into_iter().enumerate() {
            if i.consume_key(Modifiers::COMMAND, key) {
                msgs.push(Msg::SelectTab(if n == 8 { count - 1 } else { n }));
            }
        }
    });
}

/// Ctrl+Z undo; Ctrl+Y or Ctrl+Shift+Z redo (Cmd on macOS).
fn undo_shortcuts(ctx: &egui::Context, msgs: &mut Vec<Msg>) {
    use egui::Modifiers;
    ctx.input_mut(|i| {
        // Shift first: consume_key ignores extra Shift, so Ctrl+Z would also match Ctrl+Shift+Z.
        if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y) {
            msgs.push(Msg::Redo);
        } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
            msgs.push(Msg::Undo);
        }
    });
}

/// F7 / Shift+F7 and Alt+Down / Alt+Up: the next (1) or previous (-1) change in the open diff
/// (at either end, continuing into the next or previous changed file), or conflict in the
/// conflict view.
fn step_shortcuts(ctx: &egui::Context) -> Option<isize> {
    use egui::Modifiers;
    ctx.input_mut(|i| {
        // Shift first: consume_key ignores extra Shift, so F7 would also match Shift+F7.
        if i.consume_key(Modifiers::SHIFT, Key::F7) || i.consume_key(Modifiers::ALT, Key::ArrowUp) {
            Some(-1)
        } else if i.consume_key(Modifiers::NONE, Key::F7) || i.consume_key(Modifiers::ALT, Key::ArrowDown) {
            Some(1)
        } else {
            None
        }
    })
}

/// Draws one frame of the whole window from `st`, collecting the user's intents in `msgs`.
pub fn draw(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    status::status_bar(ctx, st, msgs);
    if st.prefs.is_some() {
        prefs::show(ctx, st, msgs);
    } else {
        tabs::show(ctx, st, msgs);
        toolbar::show(ctx, st, msgs);
        status::activity_log(ctx, st);
        if st.snap.is_some() {
            sidebar::show(ctx, st, msgs);
            details::show(ctx, st, cache, msgs);
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show(ctx, |ui| {
            if st.snap.is_none() {
                common::welcome(ui, st, msgs);
            } else if st.conflict.is_some() {
                conflict::show(ui, st, msgs);
            } else if st.history.is_some() {
                history::show(ui, st, msgs);
            } else if st.diff.is_some() {
                diff::show(ui, st, msgs);
            } else if st.rebase.is_some() {
                rebase::show(ui, st, cache, msgs);
            } else {
                graph::show(ui, st, cache, msgs);
            }
        });
    }
    modals::show(ctx, st, msgs);
    // After the other dialogs, so a credential prompt stacks on top of them.
    askpass::show(ctx, st, msgs);
    status::toasts(ctx, st);
}

impl eframe::App for TrellisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        while let Ok(msg) = self.results.try_recv() {
            self.dispatch(ctx, msg);
        }
        for (email, img) in self.st.view.avatars_loaded.drain(..) {
            self.cache.avatars.insert(ctx, email, img);
        }
        self.st.view.hover_tip = self.st.view.hover_tip_next.take();
        self.st.view.now = chrono::Local::now().timestamp();
        self.st.view.utc_offset = trellis_core::dates::local_offset();

        let mut msgs = vec![Msg::Tick(Instant::now())];
        self.input(ctx, &mut msgs);
        draw(ctx, &mut self.st, &mut self.cache, &mut msgs);
        let wanted = self.cache.avatars.take_wanted();
        if !wanted.is_empty() {
            msgs.push(Msg::WantAvatars(wanted));
        }
        // Views edit settings in place (Preferences); persist any change.
        msgs.push(Msg::SettingsEdited);

        for msg in msgs {
            self.dispatch(ctx, msg);
        }
        // Keep polling while work is in flight or toasts are fading.
        if self.st.job.is_some() || self.st.loading || !self.st.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(150));
        } else if self.st.settings.auto_fetch_minutes > 0 {
            ctx.request_repaint_after(Duration::from_secs(5));
        }
    }
}
