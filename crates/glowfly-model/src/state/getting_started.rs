//! Help > Getting started: a page of feature cards in the central area, opened only by the user
//! (Help, the palette, the welcome's Start step). Its cards, their groups, what each card's
//! Show me does, and when a card counts as finished: only from the repository and settings as
//! they are (a connection, a worktree, a custom command), never from counting what was used.
//! Hiding the page from the Help menu is kept in `settings.hints_seen` under `HIDDEN_ID`, so
//! "Show tips again" brings it back.

use crate::msg::{DashboardMsg, Msg, PaletteMsg, PrefsMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::PrefPage;
use crate::state::Selection;
use crate::state::palette::PaletteCommand;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

/// The id in `settings.hints_seen` that hides the page from the Help menu and the welcome.
pub const HIDDEN_ID: &str = "getting_started_hidden";

/// Whether the user hid the page from the Help menu and the welcome (the palette still finds it).
pub fn hidden(s: &Settings) -> bool {
    s.hints_seen.contains(HIDDEN_ID)
}

/// The open page (global, not per tab).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GettingStarted {
    /// Finished cards are listed too (edited by `OnboardingMsg::ShowFinished`).
    pub show_finished: bool,
}

/// The page's headings, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Everyday,
    TidyHistory,
    Parallel,
    Hosting,
    MakeItYours,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::Everyday, Group::TidyHistory, Group::Parallel, Group::Hosting, Group::MakeItYours];

    pub fn title(self, t: Lang) -> &'static str {
        match self {
            Group::Everyday => t.tr("getting_started.group.everyday"),
            Group::TidyHistory => t.tr("getting_started.group.tidy"),
            Group::Parallel => t.tr("getting_started.group.parallel"),
            Group::Hosting => t.tr("getting_started.group.hosting"),
            Group::MakeItYours => t.tr("getting_started.group.yours"),
        }
    }
}

/// One feature card. The tour comes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Tour,
    Palette,
    Blame,
    Rebase,
    Journal,
    Worktrees,
    Stash,
    Connect,
    Dashboard,
    Terminal,
    Tools,
    Commands,
    Theme,
    Keys,
}

/// What a card's Show me does once the page has closed.
pub enum ShowMe {
    /// Send this message.
    Send(Box<Msg>),
    /// Open the command palette with this text typed, listing the feature's commands.
    Search(&'static str),
    /// Nothing more: the feature is already in view.
    Nothing,
}

fn send(m: Msg) -> ShowMe {
    ShowMe::Send(Box::new(m))
}

impl Card {
    pub const ALL: [Card; 14] = [
        Card::Tour,
        Card::Palette,
        Card::Blame,
        Card::Rebase,
        Card::Journal,
        Card::Worktrees,
        Card::Stash,
        Card::Connect,
        Card::Dashboard,
        Card::Terminal,
        Card::Tools,
        Card::Commands,
        Card::Theme,
        Card::Keys,
    ];

    pub fn group(self) -> Group {
        match self {
            Card::Tour | Card::Palette | Card::Blame => Group::Everyday,
            Card::Rebase | Card::Journal => Group::TidyHistory,
            Card::Worktrees | Card::Stash => Group::Parallel,
            Card::Connect | Card::Dashboard => Group::Hosting,
            Card::Terminal | Card::Tools | Card::Commands | Card::Theme | Card::Keys => Group::MakeItYours,
        }
    }

    /// The card's title and its one sentence.
    pub fn text(self, t: Lang) -> (&'static str, &'static str) {
        match self {
            Card::Tour => (t.tr("getting_started.card.tour.title"), t.tr("getting_started.card.tour.body")),
            Card::Palette => (t.tr("getting_started.card.palette.title"), t.tr("getting_started.card.palette.body")),
            Card::Blame => (t.tr("getting_started.card.blame.title"), t.tr("getting_started.card.blame.body")),
            Card::Rebase => (t.tr("getting_started.card.rebase.title"), t.tr("getting_started.card.rebase.body")),
            Card::Journal => (t.tr("getting_started.card.journal.title"), t.tr("getting_started.card.journal.body")),
            Card::Worktrees => (t.tr("getting_started.card.worktrees.title"), t.tr("getting_started.card.worktrees.body")),
            Card::Stash => (t.tr("getting_started.card.stash.title"), t.tr("getting_started.card.stash.body")),
            Card::Connect => (t.tr("getting_started.card.connect.title"), t.tr("getting_started.card.connect.body")),
            Card::Dashboard => (t.tr("getting_started.card.dashboard.title"), t.tr("getting_started.card.dashboard.body")),
            Card::Terminal => (t.tr("getting_started.card.terminal.title"), t.tr("getting_started.card.terminal.body")),
            Card::Tools => (t.tr("getting_started.card.tools.title"), t.tr("getting_started.card.tools.body")),
            Card::Commands => (t.tr("getting_started.card.commands.title"), t.tr("getting_started.card.commands.body")),
            Card::Theme => (t.tr("getting_started.card.theme.title"), t.tr("getting_started.card.theme.body")),
            Card::Keys => (t.tr("getting_started.card.keys.title"), t.tr("getting_started.card.keys.body")),
        }
    }

    /// Whether the repository or the settings show the feature in use already.
    pub fn done(self, st: &AppState) -> bool {
        let s = &st.settings;
        let snap = st.snap.as_ref();
        match self {
            Card::Worktrees => snap.is_some_and(|s| s.worktrees.len() > 1),
            Card::Stash => snap.is_some_and(|s| !s.stashes.is_empty()),
            Card::Connect => !s.connections.is_empty(),
            Card::Tools => {
                let x = &s.external_tools;
                [&x.editor, &x.diff_tool, &x.merge_tool].iter().any(|p| !p.trim().is_empty())
            }
            Card::Commands => !st.custom_commands.user.is_empty() || !st.custom_commands.repo.is_empty(),
            Card::Theme => s.theme != Settings::default().theme,
            Card::Keys => !s.keybindings.is_empty(),
            Card::Tour | Card::Palette | Card::Blame | Card::Rebase | Card::Journal | Card::Dashboard | Card::Terminal => false,
        }
    }

    /// What Show me does now, or why it can't (shown on the disabled button).
    pub fn show_me(self, st: &AppState) -> Result<ShowMe, &'static str> {
        let t = st.settings.lang();
        let repo = || st.snap.is_some().then_some(()).ok_or(t.tr("getting_started.needs_repo"));
        let prefs = |page| Ok(send(Msg::Prefs(PrefsMsg::OpenPrefs(page))));
        match self {
            Card::Tour => repo().map(|_| send(Msg::Onboarding(OnboardingMsg::StartTour))),
            Card::Palette => Ok(send(Msg::Palette(PaletteMsg::TogglePalette))),
            Card::Blame => repo().map(|_| send(Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Blame)))),
            Card::Rebase => {
                repo()?;
                match st.session.selection {
                    Selection::Commit(_) => Ok(ShowMe::Search("rebase")),
                    _ => Err(t.tr("getting_started.needs_commit")),
                }
            }
            Card::Journal => repo().map(|_| send(Msg::Journal(crate::journal::JournalMsg::Open))),
            Card::Worktrees => repo().map(|_| ShowMe::Search("worktree")),
            Card::Stash => repo().map(|_| ShowMe::Search("stash")),
            Card::Connect => prefs(PrefPage::Integrations),
            Card::Dashboard => Ok(send(Msg::Dashboard(DashboardMsg::Open))),
            Card::Terminal => {
                repo()?;
                let open = Msg::Terminal(crate::terminal::TerminalMsg::Toggle);
                Ok(if st.terminal.open { ShowMe::Nothing } else { send(open) })
            }
            Card::Tools => prefs(PrefPage::Tools),
            Card::Commands => repo().map(|_| ShowMe::Search("custom command")),
            Card::Theme => prefs(PrefPage::Ui),
            Card::Keys => prefs(PrefPage::Keyboard),
        }
    }
}

/// The page's groups with the cards it lists (finished ones only when asked), groups left
/// empty dropped; each card with whether it is finished.
pub fn groups(st: &AppState, show_finished: bool) -> Vec<(Group, Vec<(Card, bool)>)> {
    Group::ALL
        .iter()
        .map(|&g| {
            let cards = Card::ALL.iter().filter(|c| c.group() == g).map(|&c| (c, c.done(st)));
            (g, cards.filter(|(_, done)| show_finished || !done).collect::<Vec<_>>())
        })
        .filter(|(_, cards)| !cards.is_empty())
        .collect()
}

/// How many cards are finished, of how many.
pub fn progress(st: &AppState) -> (usize, usize) {
    (Card::ALL.iter().filter(|c| c.done(st)).count(), Card::ALL.len())
}
