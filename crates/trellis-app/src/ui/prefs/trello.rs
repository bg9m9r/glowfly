//! Trello's parts of Preferences: the API key and token links on its Integrations page, and
//! the Board picker under Repo-specific preferences > Issue tracker.

use super::{help, row};
use crate::msg::{ForgeMsg, Msg};
use crate::state::trello::BoardList;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText, Ui};
use trellis_core::forge::{self, Account, Provider, Target};
use trellis_core::settings::TrackerChoice;

/// "Get your API key": the Power-Up admin page, where Trello shows the member's API key.
pub(super) fn key_link(ui: &mut Ui, msgs: &mut Vec<Msg>) {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    if let Some(url) = Target::token_url(&account)
        && ui.link("Get your API key").clicked()
    {
        msgs.push(Msg::OpenUrl(url));
    }
}

/// Trello's pages that show a new token for the key typed in: a read-only one, which lists
/// cards, and one that can also create cards.
pub(super) fn token_link(ui: &mut Ui, key: &str, msgs: &mut Vec<Msg>) {
    for (text, write) in [("Allow a read-only token", false), ("Allow a token that can create cards", true)] {
        let url = forge::trello_authorize_url(key, write);
        let r = ui.add_enabled(url.is_some(), egui::Link::new(text));
        let r = r.on_disabled_hover_text("Enter your API key first");
        if r.clicked()
            && let Some(url) = url
        {
            msgs.push(Msg::OpenUrl(url));
        }
    }
}

/// The Board row while Trello is the repository's tracker; an empty `board` is all boards.
pub(super) fn board_row(ui: &mut Ui, account: &Account, board: &str, board_name: &str, list: &BoardList, msgs: &mut Vec<Msg>) {
    row(ui, "Board", |ui| {
        let choose = |id: &str, name: &str| {
            Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Trello {
                account: account.clone(),
                board: id.to_owned(),
                board_name: name.to_owned(),
            }))
        };
        let selected = if board.is_empty() { "All boards" } else { board_name };
        egui::ComboBox::from_id_salt("trello-board").selected_text(selected).width(320.0).show_ui(ui, |ui| {
            if ui.selectable_label(board.is_empty(), "All boards").clicked() && !board.is_empty() {
                msgs.push(choose("", ""));
            }
            for b in &list.boards {
                if ui.selectable_label(b.id == board, &b.name).clicked() && b.id != board {
                    msgs.push(choose(&b.id, &b.name));
                }
            }
        });
        if list.loading {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0));
                ui.label(RichText::new("Loading boards…").size(type_scale::SMALL).color(color::weak()));
            });
        } else if let Some(e) = &list.error {
            form_message::line(ui, Message::error(e.as_str()));
            if ui.add(buttons::secondary("Try again").small()).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::LoadTrelloBoards(account.clone())));
            }
        } else if !board.is_empty() && list.account.as_ref() == Some(account) && !list.has(board) {
            form_message::line(ui, Message::warning("This board is closed or no longer shared with you."));
        }
        help(
            ui,
            "The ISSUES list shows open cards assigned to you on this board. Card links (trello.com/c/…) in commit messages, \
             and branch names that start with a card id, link to Trello either way.",
        );
    });
}
