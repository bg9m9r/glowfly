//! The Commands submenu's items for a commit, branch or file menu: which commands show there,
//! whether each can run from that row (and why not), and the hover text (the filled-in
//! command, or that it asks to trust the repository first). The views only draw them.

use super::{CustomCommands, missing_text, vars_for};
use crate::custom_commands::{CommandRef, CustomCommandMsg, Target};
use crate::msg::Msg;
use glowfly_core::custom_commands::{Command, Place, Source};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// One command in a menu, and whether it waits for the trust prompt.
struct MenuCommand<'a> {
    cref: CommandRef,
    cmd: &'a Command,
    held: bool,
}

/// The commands the menus of one repository offer.
pub struct MenuCommands<'a> {
    list: Vec<MenuCommand<'a>>,
    snap: Option<&'a Snapshot>,
    t: Lang,
}

/// A drawn item: its label, the message it sends, why it is off, and its hover text.
pub struct MenuItem {
    pub label: String,
    pub msg: Msg,
    pub blocked: Option<&'static str>,
    pub tip: Option<String>,
}

impl<'a> MenuCommands<'a> {
    /// No commands (tests of menus that don't show them).
    pub fn none() -> Self {
        Self { list: Vec::new(), snap: None, t: Lang::En }
    }

    /// `cc`'s commands for the repository `snap`; `trusted`: its own commands run without the
    /// trust prompt.
    pub fn of(cc: &'a CustomCommands, snap: &'a Snapshot, trusted: bool, t: Lang) -> Self {
        let list = cc
            .listed(Some(&snap.workdir))
            .map(|(source, cmd)| MenuCommand {
                cref: CommandRef { source, name: cmd.name.clone() },
                cmd,
                held: source == Source::Repo && !trusted,
            })
            .collect();
        Self { list, snap: Some(snap), t }
    }

    /// The active repository's commands in `st`.
    pub fn for_state(st: &'a super::super::AppState) -> Self {
        match &st.snap {
            Some(snap) => Self::of(&st.custom_commands, snap, super::repo_trusted(st), st.settings.lang()),
            None => Self::none(),
        }
    }

    /// The items for the menu at `place` of the row `at`; empty when no command shows there.
    /// `windows` decides the filled-in command's spelling.
    pub fn items(&self, place: Place, at: &Target, windows: bool) -> Vec<MenuItem> {
        let Some(snap) = self.snap else { return Vec::new() };
        let vars = vars_for(snap, at, None, None);
        let v = vars.as_vars();
        self.list
            .iter()
            .filter(|m| m.cmd.shows_in(place))
            .map(|m| {
                let msg = Msg::CustomCommand(CustomCommandMsg::Run { command: m.cref.clone(), at: at.clone(), confirmed: false });
                let (blocked, tip) = match m.cmd.expand(&v, windows) {
                    Err(missing) => (Some(missing_text(self.t, missing)), None),
                    Ok(_) if m.held => (None, Some(self.t.tr("commands.menu.held_tip").to_owned())),
                    Ok(e) => (None, Some(e.preview())),
                };
                MenuItem { label: m.cmd.name.clone(), msg, blocked, tip }
            })
            .collect()
    }
}
