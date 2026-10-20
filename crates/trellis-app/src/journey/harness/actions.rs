//! What a user does often in a journey: the palette, Refresh, committing everything, and a
//! branch's menu in the sidebar.

use super::super::find::{self, Pick, Region};
use super::Journey;
use eframe::egui;
use trellis_core::i18n::Lang;

impl Journey {
    /// Runs a command from the palette: opens it, types `query` and clicks the row `row`.
    #[track_caller]
    pub fn run_command(&mut self, query: &str, row: &str) {
        self.open_palette(query);
        self.click(row, Region::Palette, Pick::Only);
    }

    /// Presses the palette's key and types `query` into its field.
    #[track_caller]
    pub fn open_palette(&mut self, query: &str) {
        self.toasts.clear();
        let chord = self.state().keymap.chords(crate::keymap::BindingId::Palette)[0];
        let key = egui::Key::from_name(chord.key.config()).expect("the palette's key is an egui key");
        let (m, mac) = (chord.mods, cfg!(target_os = "macos"));
        let command = egui::Modifiers { ctrl: !mac, mac_cmd: mac, command: true, ..egui::Modifiers::NONE };
        let mut modifiers = if m.command { command } else { egui::Modifiers::NONE };
        modifiers.ctrl |= m.ctrl;
        (modifiers.shift, modifiers.alt) = (m.shift, m.alt);
        self.h.press_key_modifiers(modifiers, key);
        self.settle();
        {
            let Some(node) = find::field_in(&self.h, Region::Palette) else {
                panic!("{}", self.missing("text field", "palette search", Region::Palette, Pick::First))
            };
            node.type_text(query);
        }
        self.settle();
    }

    /// Refresh in the toolbar, as after editing files outside the app.
    pub fn refresh(&mut self) {
        self.click("Refresh the repository", Region::Toolbar, Pick::Only);
    }

    /// Selects the working tree's row in the graph.
    pub fn select_wip(&mut self) {
        self.click("Uncommitted changes", Region::Central, Pick::Only);
    }

    /// Stages everything and commits it as `summary` from the details panel.
    pub fn commit_all(&mut self, summary: &str) {
        self.refresh();
        self.select_wip();
        self.click("Stage all", Region::Details, Pick::Only);
        self.type_into("Commit summary", summary);
        // The button names the staged count; a bare "Commit " would also match "Commit summary".
        let n = self.state().snap.as_ref().map_or(0, |s| s.staged.len());
        let label = Lang::En.trn("commit.button.commit", n, &[("push", &""), ("skip", &"")]);
        self.click_containing(&label, Region::Details, Pick::Only);
        self.toast(true, "Commit succeeded");
    }

    /// The sidebar row of local branch `name` (checked out or not); local rows come before the
    /// remotes' rows of the same name.
    pub fn local_branch(&self, name: &str) -> String {
        let head = format!("✔  {name}");
        if find::count(&self.h, &head, true, Region::Sidebar) > 0 { head } else { name.to_owned() }
    }

    /// Right-clicks local branch `name` in the sidebar and picks `item` from its menu.
    pub fn branch_menu(&mut self, name: &str, item: &str) {
        let row = self.local_branch(name);
        self.right_click(&row, Region::Sidebar, Pick::First);
        self.menu(item);
    }
}
