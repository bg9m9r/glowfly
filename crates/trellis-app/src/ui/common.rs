//! Small view helpers shared across regions.

use super::theme::color;
use super::widgets;
use crate::msg::CloneMsg;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::{AppState, ForgeState, PrefPage, Status};
use eframe::egui::{self, Color32, RichText, pos2, vec2};
use git2::{Oid, RepositoryState};
use trellis_core::forge::{self, RefStyle};

const LANE_COLORS: [Color32; 10] = [
    Color32::from_rgb(0x1f, 0xb5, 0xc9),
    Color32::from_rgb(0x3d, 0x7d, 0xf7),
    Color32::from_rgb(0x9b, 0x5d, 0xe0),
    Color32::from_rgb(0xd0, 0x4f, 0xb8),
    Color32::from_rgb(0xe8, 0x4a, 0x7a),
    Color32::from_rgb(0xe8, 0x5c, 0x3a),
    Color32::from_rgb(0xf0, 0x8d, 0x2e),
    Color32::from_rgb(0xe8, 0xc4, 0x30),
    Color32::from_rgb(0x7b, 0xc9, 0x3f),
    Color32::from_rgb(0x2e, 0xc2, 0x95),
];

/// The height of a `ui.horizontal` row of labels and buttons, for lists drawn with
/// `ScrollArea::show_rows` (only the rows in view are laid out).
pub fn widget_row_height(ui: &egui::Ui) -> f32 {
    // A laid-out line, not the font's row height: galleys are rounded to whole points.
    let styles = [egui::TextStyle::Body, egui::TextStyle::Button, egui::TextStyle::Monospace];
    let line = |t: &egui::TextStyle| ui.fonts(|f| f.layout_no_wrap("Ag".into(), t.resolve(ui.style()), Color32::WHITE).size().y);
    let text = styles.iter().map(line).fold(0.0, f32::max);
    let s = ui.spacing();
    s.interact_size.y.max(text + 2.0 * s.button_padding.y)
}

pub fn lane_color(i: usize) -> Color32 {
    LANE_COLORS[i % LANE_COLORS.len()]
}

pub fn status_color(c: char) -> Color32 {
    match c {
        'A' | '?' => color::green(),
        'D' => color::red(),
        'R' => color::blue(),
        '!' => color::amber(),
        'M' => color::amber(),
        _ => color::weak(),
    }
}

/// Untracked files show as additions.
pub fn status_label(c: char) -> char {
    if c == '?' { 'A' } else { c }
}

pub fn state_color(state: &str) -> Color32 {
    match state.to_ascii_lowercase().as_str() {
        "new" | "to do" | "proposed" => color::weak(),
        "active" | "in progress" | "committed" | "doing" => color::blue(),
        "resolved" | "done" | "closed" => color::green(),
        _ => color::amber(),
    }
}

pub fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// One line of text cut to `max_w` with an ellipsis, vertically centred on `pos.y` from
/// `pos.x`; returns its width.
pub fn paint_cut_line(ui: &egui::Ui, pos: egui::Pos2, text: &str, font: egui::FontId, c: Color32, max_w: f32) -> f32 {
    if max_w <= 0.0 {
        return 0.0;
    }
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, c);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
    let galley = ui.fonts(|f| f.layout_job(job));
    let w = galley.size().x;
    ui.painter().galley(pos2(pos.x, pos.y - galley.size().y / 2.0), galley, c);
    w
}

pub fn menu_item(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, m: Msg) {
    if ui.button(label).clicked() {
        msgs.push(m);
        ui.close_menu();
    }
}

/// A menu item that is disabled, with the reason on hover, while `blocked` is set.
pub fn menu_item_if(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, blocked: Option<&str>, m: Msg) {
    let mut r = ui.add_enabled(blocked.is_none(), egui::Button::new(label));
    if let Some(why) = blocked {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked() {
        msgs.push(m);
        ui.close_menu();
    }
}

/// Small caret drawn after a menu button (the font has no ▾ glyph).
pub fn caret_after(ui: &mut egui::Ui, r: &egui::Response) {
    let c = pos2(r.rect.right() + 6.0, r.rect.center().y + 1.0);
    widgets::chevron(ui.painter(), c, true, color::weak());
}

/// A clickable `#123 · Type · State  Title` chip; shows just the key until the issue is loaded.
pub fn issue_chip(ui: &mut egui::Ui, fs: &ForgeState, key: &str, msgs: &mut Vec<Msg>) {
    let tag = fs.mention(key);
    let text = match fs.cache.get(key) {
        Some(i) => RichText::new(format!("{tag}  {} · {}  {}", i.kind, i.state, i.title)).color(color::text()),
        None => RichText::new(tag).color(color::blue()),
    };
    let r = ui.add(egui::Button::new(text.size(12.5)).fill(color::input()).truncate());
    if r.clicked() {
        msgs.push(Msg::OpenUrl(fs.issue_url(key)));
    }
    r.on_hover_text(format!("Open in {}", fs.tracker_name()));
}

pub fn hint_row(ui: &mut egui::Ui, text: String) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        ui.label(RichText::new(text).size(11.5).color(color::faint()));
    });
    ui.add_space(4.0);
}

pub fn forge_error_rows(ui: &mut egui::Ui, error: &str, msgs: &mut Vec<Msg>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        ui.label(RichText::new(error).color(color::red()).size(12.0));
    });
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        if ui.small_button("Sign in").clicked() {
            msgs.push(Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }));
        }
        if ui.small_button("Connect…").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
        }
    });
    ui.add_space(4.0);
}

pub fn status_label_ui(ui: &mut egui::Ui, status: &Status, ok_text: &str, msgs: &mut Vec<Msg>) {
    match status {
        Status::Ok => {
            ui.label(RichText::new(format!("· {ok_text}")).color(color::weak()));
        }
        Status::Error(e) => {
            ui.label(RichText::new(format!("· {e}")).color(color::red()));
            if ui.link("Sign in").clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }));
            }
            if ui.link("Connect…").clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
            }
        }
        Status::Idle => {}
    }
}

/// Splits "12, #13 AB#14 PROJ-5" into issue keys; a pasted Trello card link becomes its id.
pub fn parse_keys(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split([',', ' ', ';']) {
        let card = forge::issue_refs(RefStyle::Trello, part).into_iter().next();
        let p = card.as_deref().unwrap_or(part.trim());
        let p = p.strip_prefix("AB#").or_else(|| p.strip_prefix("ab#")).unwrap_or(p).trim_start_matches('#');
        if !p.is_empty() && !out.iter().any(|k| k == p) {
            out.push(p.to_owned());
        }
    }
    out
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub type StateAction = (&'static str, &'static [&'static str]);

const REBASE_ACTIONS: &[StateAction] =
    &[("Continue", &["rebase", "--continue"]), ("Skip commit", &["rebase", "--skip"]), ("Abort rebase", &["rebase", "--abort"])];

/// The in-progress operation name and its Continue/Skip/Abort commands.
pub fn state_actions(state: RepositoryState) -> Option<(&'static str, &'static [StateAction])> {
    use RepositoryState::*;
    Some(match state {
        Merge => ("Merge", &[("Abort merge", &["merge", "--abort"])]),
        Rebase | RebaseMerge => ("Rebase", REBASE_ACTIONS),
        RebaseInteractive => ("Interactive rebase", REBASE_ACTIONS),
        CherryPick | CherryPickSequence => {
            ("Cherry-pick", &[("Continue", &["cherry-pick", "--continue"]), ("Abort cherry-pick", &["cherry-pick", "--abort"])])
        }
        Revert | RevertSequence => ("Revert", &[("Continue", &["revert", "--continue"]), ("Abort revert", &["revert", "--abort"])]),
        ApplyMailbox | ApplyMailboxOrRebase => ("Apply mailbox", &[("Abort", &["am", "--abort"])]),
        Bisect => ("Bisect", &[("Reset bisect", &["bisect", "reset"])]),
        Clean => return None,
    })
}

/// Shown when no repository is open.
pub fn welcome(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.2);
        ui.label(RichText::new("Trellis").size(40.0).strong().color(color::text()));
        ui.label(RichText::new("A fast, native Git client").color(color::weak()));
        ui.add_space(20.0);
        if st.loading {
            ui.add(egui::Spinner::new());
            return;
        }
        let size = vec2(260.0, 34.0);
        if ui.add(egui::Button::new("Open a repository").min_size(size)).clicked() {
            msgs.push(Msg::Repo(RepoMsg::PickRepo));
        }
        if ui.add(egui::Button::new("Clone a repository").min_size(size)).clicked() {
            msgs.push(Msg::Clone(CloneMsg::Dialog));
        }
        if ui.add(egui::Button::new("Start a new repository").min_size(size)).clicked() {
            msgs.push(Msg::Repo(RepoMsg::InitRepo));
        }
        super::repo_list::welcome_sections(ui, st, msgs);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_issue_key_lists() {
        assert_eq!(parse_keys("12, #13 AB#14  PROJ-5,12"), ["12", "13", "14", "PROJ-5"]);
        assert_eq!(parse_keys("https://trello.com/c/AbCd1234/3-x, Zz9Yy8Xx"), ["AbCd1234", "Zz9Yy8Xx"]);
        assert_eq!(parse_keys("12, PROJ-5"), ["12", "PROJ-5"]);
        assert_eq!(capitalize("work item"), "Work item");
    }
}
