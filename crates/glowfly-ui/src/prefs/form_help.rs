//! Preferences help lines: 13 px weak text set 1.45 high, broken between words at no more than
//! `MEASURE` characters a line. Help that would take more than two lines shows its first
//! sentence on one line, then "Learn more", which opens the whole text in place, ending with
//! "Show less". Which rows are open is view state that resets when the page changes.

use crate::theme::{color, type_scale};
use eframe::egui::{self, Id, Sense, Ui, text::LayoutJob};
use glowfly_core::i18n::Lang;
use std::collections::HashSet;
use std::sync::Arc;

/// The most characters a help line holds.
pub const MEASURE: usize = 64;
/// Help taking more lines than this collapses to its first sentence.
const MAX_LINES: usize = 2;

/// The open help texts, by id, in egui's memory.
fn open_key() -> Id {
    Id::new("prefs-help-open")
}

/// The language the help's links are in, set by `set_lang` each frame.
fn lang_key() -> Id {
    Id::new("prefs-help-lang")
}

pub fn set_lang(ctx: &egui::Context, t: Lang) {
    ctx.data_mut(|d| d.insert_temp(lang_key(), t));
}

fn lang(ctx: &egui::Context) -> Lang {
    ctx.data(|d| d.get_temp::<Lang>(lang_key())).unwrap_or(Lang::En)
}

/// Closes every opened help text (when the page changes).
pub fn close_all(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove_temp::<Arc<HashSet<Id>>>(open_key()));
}

pub fn help(ui: &mut Ui, text: &str) {
    let lines = wrap(text, MEASURE);
    if lines.len() <= MAX_LINES {
        block(ui, &lines);
        return;
    }
    let t = lang(ui.ctx());
    // By its text: the same help opens wherever it shows.
    let id = Id::new(("prefs-help", text));
    let mut open_set = ui.ctx().data(|d| d.get_temp::<Arc<HashSet<Id>>>(open_key())).unwrap_or_default();
    let open = open_set.contains(&id);
    let clicked = if open {
        text_then_link(ui, &lines, t.tr("prefs.help.show_less"))
    } else {
        let link = t.tr("prefs.help.learn_more");
        let first = first_line(text, MEASURE.saturating_sub(link.chars().count() + 1));
        text_then_link(ui, std::slice::from_ref(&first), link)
    };
    if clicked {
        let set = Arc::make_mut(&mut open_set);
        if !set.remove(&id) {
            set.insert(id);
        }
        ui.ctx().data_mut(|d| d.insert_temp(open_key(), open_set));
    }
}

/// `lines` as one block of help text, named by its words for screen readers.
fn block(ui: &mut Ui, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let mut job = LayoutJob::default();
    let format = egui::TextFormat {
        font_id: type_scale::font(type_scale::HELP),
        color: color::weak(),
        line_height: Some(type_scale::HELP_LINE),
        ..Default::default()
    };
    job.append(&lines.join("\n"), 0.0, format);
    job.wrap.max_width = ui.available_width();
    let galley = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, lines.join(" ")));
    ui.painter().galley(rect.min, galley, color::weak());
}

/// `lines`, with `link` after the last of them (or under it when it doesn't fit the measure).
/// Returns whether the link was clicked.
fn text_then_link(ui: &mut Ui, lines: &[String], link: &str) -> bool {
    let Some((last, before)) = lines.split_last() else { return false };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let inline = last.chars().count() + 1 + link.chars().count() <= MEASURE;
        if inline {
            block(ui, before);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                block(ui, std::slice::from_ref(last));
                link_button(ui, link)
            })
            .inner
        } else {
            block(ui, lines);
            link_button(ui, link)
        }
    })
    .inner
}

/// The accent link, not selectable as text: a selectable link takes a press as the start of a
/// text selection.
fn link_button(ui: &mut Ui, text: &str) -> bool {
    ui.scope(|ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui.link(egui::RichText::new(text).size(type_scale::HELP)).clicked()
    })
    .inner
}

/// `text` broken between words into lines of at most `measure` characters (a longer word gets
/// a line of its own). Line breaks in `text` are kept.
pub fn wrap(text: &str, measure: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let len = line.chars().count();
            if len > 0 && len + 1 + word.chars().count() > measure {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

/// The first sentence of `text`, cut between words with "…" when it is longer than `room`
/// characters.
pub fn first_line(text: &str, room: usize) -> String {
    let sentence = match text.find(". ") {
        Some(i) => &text[..=i],
        None => text,
    };
    if sentence.chars().count() <= room {
        return sentence.to_owned();
    }
    let mut out = String::new();
    for word in sentence.split_whitespace() {
        if out.chars().count() + usize::from(!out.is_empty()) + word.chars().count() + 1 > room {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.trim_end_matches([',', ';', ':']).to_owned() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_breaks_between_words_within_the_measure() {
        let text = "Glowfly asks GitHub's releases API for the newest release of its repository once a day. GitHub sees your IP address.";
        let lines = wrap(text, MEASURE);
        assert!(lines.iter().all(|l| l.chars().count() <= MEASURE), "{lines:?}");
        assert_eq!(lines.join(" "), text);
        assert_eq!(wrap("one\ntwo", MEASURE), ["one", "two"]);
    }

    #[test]
    fn the_collapsed_line_is_the_first_sentence_cut_to_fit() {
        assert_eq!(first_line("Short one. And more after it.", 40), "Short one.");
        let cut = first_line("A first sentence that goes on for far too long to fit on one line at all. Then more.", 40);
        assert!(cut.ends_with('…') && cut.chars().count() <= 40, "{cut}");
        assert!(cut.starts_with("A first sentence"));
    }
}
