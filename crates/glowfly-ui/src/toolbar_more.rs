//! The toolbar's ⋯ More button, shown only while something is folded (`toolbar_fold.rs`), on
//! its own group where the folded groups were, and its menu: one section per folded part in
//! the toolbar's order, parted by separators. Each item is the button's caption (Undo and Redo
//! say what they would undo) with its key; Terminal is checked while its panel is open, and a
//! button that can't be used now is a blocked item with the same reason. The pull request
//! button is its action, then its menu as a submenu, so a click runs exactly one thing.

use super::fold::{ORDER, Part, is_folded};
use super::groups::{create_pr, merge_requests};
use crate::keymap::BindingId;
use crate::msg::{BranchMsg, JobMsg, Msg, PaletteMsg, PrefsMsg};
use crate::state::tip::Tip;
use crate::state::toolbar_tips as rules;
use crate::state::{AppState, ForgeState, PrefPage};
use crate::ui::widgets::menu::{self, popup_below, separator, submenu};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, PopupCloseBehavior};
use glowfly_core::repo::Snapshot;
use glowfly_model::terminal::TerminalMsg;

/// The More menu's least width.
const MENU_W: f32 = 240.0;

/// The More button's caption.
pub(super) fn caption(st: &AppState) -> &'static str {
    st.settings.lang().tr("toolbar.more.button")
}

/// The More button's group, for the first `k` parts of `ORDER` folded.
pub(super) fn show(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, host: Option<&ForgeState>, k: usize, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let name = t.tr("toolbar.more.name");
    let popup = ui.make_persistent_id("toolbar-more");
    widgets::tool_group(ui, |ui| {
        let r = widgets::tool_toggle(ui, Icon::More, caption(st), name, ui.memory(|m| m.is_popup_open(popup)), st.settings.toolbar_labels);
        let r = widgets::tooltip::attach(r, &st.keymap, t, || Tip::new(t.tr("toolbar.more.tip")));
        if r.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        popup_below(ui, popup, &r, PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(MENU_W);
            let before = msgs.len();
            items(ui, st, snap, host, k, msgs);
            if msgs.len() > before {
                ui.memory_mut(|m| m.close_popup());
            }
        });
    });
}

/// The folded parts' items, a separator between parts.
fn items(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, host: Option<&ForgeState>, k: usize, msgs: &mut Vec<Msg>) {
    // In the toolbar's order, left to right.
    const SHOWN: [Part; 6] = [Part::Undo, Part::Branch, Part::Pr, Part::Files, Part::Commands, Part::Settings];
    debug_assert!(SHOWN.iter().all(|p| ORDER.contains(p)));
    let mut first = true;
    for part in SHOWN.into_iter().filter(|&p| is_folded(p, k)) {
        if part == Part::Pr && host.is_none() {
            continue;
        }
        // Commands and Settings come from one group, so no line between them.
        if !first && part != Part::Settings {
            separator(ui);
        }
        first = false;
        part_items(ui, st, snap, host, part, msgs);
    }
}

fn part_items(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, host: Option<&ForgeState>, part: Part, msgs: &mut Vec<Msg>) {
    let (t, idle) = (st.settings.lang(), st.job.is_none());
    let key = |id| st.keymap.hint(id).unwrap_or_default();
    let item = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>, label: &str, id: Option<BindingId>, why: Option<&str>, m: Msg| {
        if menu::item(label).shortcut(id.map(key).unwrap_or_default()).blocked(why).show(ui).clicked() {
            msgs.push(m);
        }
    };
    match part {
        Part::Undo => {
            let (label, why) = st.session.undo.undo_entry(t, idle);
            item(ui, msgs, &label, Some(BindingId::Undo), why.as_deref(), Msg::Job(JobMsg::Undo));
            let (label, why) = st.session.undo.redo_entry(t, idle);
            item(ui, msgs, &label, Some(BindingId::Redo), why.as_deref(), Msg::Job(JobMsg::Redo));
        }
        Part::Branch => {
            let new = Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() });
            item(ui, msgs, t.tr("toolbar.branch.button"), Some(BindingId::CreateBranch), rules::branch_blocked(st).as_deref(), new);
            let stash = Msg::git("Stash", &["stash", "push", "-u"]);
            item(ui, msgs, t.tr("toolbar.stash.button"), None, rules::stash_blocked(st).as_deref(), stash);
            let why = rules::pop_blocked(st);
            let r = menu::item(t.tr("toolbar.pop.button")).blocked(why.as_deref()).show(ui);
            if r.clicked() {
                msgs.extend(crate::state::stash_confirm::pop_latest(snap, t));
            }
        }
        Part::Pr => {
            let Some(fs) = host else { return };
            let service = fs.host_provider().map(glowfly_core::forge::Provider::name).unwrap_or_default();
            let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
            let (label, options) = if merge_requests(fs) {
                (t.trf("toolbar.pr.tip_merge_request", args), t.tr("toolbar.more.merge_request_options"))
            } else {
                (t.trf("toolbar.pr.tip_pull_request", args), t.tr("toolbar.more.pull_request_options"))
            };
            let why = rules::pr_blocked(st);
            if menu::item(label).blocked(why.as_deref()).show(ui).clicked() {
                msgs.extend(create_pr(snap));
            }
            submenu(ui, options, |ui| crate::ui::pr_menu_items::pr_menu(ui, t, snap, fs, idle, msgs));
        }
        Part::Files => {
            let open = st.terminal.open;
            let r = menu::item(t.tr("toolbar.terminal.button")).check(open).shortcut(key(BindingId::ToggleTerminal)).show(ui);
            if r.clicked() {
                msgs.push(Msg::Terminal(TerminalMsg::Toggle));
            }
            item(ui, msgs, t.tr("toolbar.folder.button"), None, None, Msg::OpenFolder(snap.workdir.clone()));
        }
        Part::Commands => {
            item(ui, msgs, t.tr("toolbar.commands.button"), Some(BindingId::Palette), None, Msg::Palette(PaletteMsg::TogglePalette))
        }
        Part::Settings => item(
            ui,
            msgs,
            t.tr("toolbar.settings.button"),
            Some(BindingId::Prefs),
            None,
            Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)),
        ),
    }
}
