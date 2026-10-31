//! Markdown text runs (`ui/markdown.rs`): one label per run in its style, wrapped as one
//! paragraph. Links show their address on hover; only an https link opens (through
//! `Msg::OpenUrl`), since a relative or other link in a description has nowhere safe to go.
//! Runs sit edge to edge, so an italic run is followed by room for its slant (`lean`), which egui
//! draws past the run's end, and a code run's background keeps `CODE_ROOM` from text it touches.

use super::theme::{color, type_scale};
use crate::msg::Msg;
use eframe::egui::{self, Color32, CursorIcon, FontId, FontSelection, RichText, TextWrapMode, Ui, WidgetText};
use glowfly_core::markdown::Inline;

pub(crate) const ONLY_HTTPS: &str = "Only https links open in the browser";
/// Between a code run's background and punctuation or a letter right against it ("`x`.").
const CODE_ROOM: f32 = 2.0;

/// How a block draws its runs.
#[derive(Clone, Copy)]
pub(crate) struct Look {
    /// A type step.
    pub size: f32,
    /// Every run SemiBold (a heading).
    pub strong: bool,
    pub color: Color32,
}

/// Lays `runs` out as one paragraph wrapped to the width (`wrap`), or on one line; a hard
/// break starts a new line.
pub(crate) fn runs(ui: &mut Ui, runs: &[Inline], look: Look, wrap: bool, msgs: &mut Vec<Msg>) {
    let add = |ui: &mut Ui, msgs: &mut Vec<Msg>| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let parts = pieces(runs);
        for (i, &(run, part, new_line)) in parts.iter().enumerate() {
            if new_line {
                ui.end_row();
            }
            if part.is_empty() {
                continue;
            }
            let before = if new_line { None } else { i.checked_sub(1).and_then(|j| parts[j].1.chars().next_back()) };
            let after = parts.get(i + 1).filter(|p| !p.2).and_then(|p| p.1.chars().next());
            one(ui, run, part, look, (before, after), msgs);
        }
    };
    if wrap {
        ui.horizontal_wrapped(|ui| add(ui, msgs));
    } else {
        ui.horizontal(|ui| add(ui, msgs));
    }
}

/// The runs' text split at hard breaks: each part with its run and whether a new line starts
/// before it.
fn pieces(runs: &[Inline]) -> Vec<(&Inline, &str, bool)> {
    runs.iter().flat_map(|run| run.text.split('\n').enumerate().map(move |(i, part)| (run, part, i > 0))).collect()
}

/// Draws one part of a run; `near` is the character drawn right before it and right after it on
/// the same line, if any.
fn one(ui: &mut Ui, run: &Inline, text: &str, look: Look, near: (Option<char>, Option<char>), msgs: &mut Vec<Msg>) {
    let s = run.style;
    let strong = look.strong || s.strong;
    let font = if s.code {
        type_scale::mono(look.size)
    } else if strong {
        type_scale::strong(ui, look.size)
    } else {
        type_scale::font(look.size)
    };
    let italic = s.emphasis || s.image;
    let touches = |c: Option<char>| s.code && c.is_some_and(|c| !c.is_whitespace());
    let room = if italic { lean(ui, text, &font) } else { 0.0 } + if touches(near.1) { CODE_ROOM } else { 0.0 };
    if touches(near.0) {
        ui.add_space(CODE_ROOM);
    }
    let mut rich = RichText::new(text).font(font);
    if s.code {
        rich = rich.background_color(color::raised());
    }
    if italic {
        rich = rich.italics();
    }
    if s.strike {
        rich = rich.strikethrough();
    }
    let col = if s.image && run.link.is_none() {
        color::weak()
    } else if strong && look.color == color::text() {
        color::strong()
    } else {
        look.color
    };
    add(ui, run, rich, col, msgs);
    if room > 0.0 {
        ui.add_space(room);
    }
}

/// How far `text`'s last word leans past its end in italics. egui slants italics by shifting
/// each glyph's top right by a quarter of its height without widening it, so the run after an
/// italic one (" the", ".") started under the slant: "*try* the" read "trythe".
fn lean(ui: &Ui, text: &str, font: &FontId) -> f32 {
    let word = text.rsplit(char::is_whitespace).next().unwrap_or_default();
    if word.is_empty() {
        return 0.0;
    }
    let text = WidgetText::from(RichText::new(word).font(font.clone()).italics());
    let galley = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, FontSelection::Default);
    (galley.mesh_bounds.max.x - galley.rect.max.x).max(0.0)
}

/// Adds the styled run as a label in `col`, or as a link when it has one.
fn add(ui: &mut Ui, run: &Inline, rich: RichText, col: Color32, msgs: &mut Vec<Msg>) {
    let Some(url) = &run.link else {
        ui.label(rich.color(col));
        return;
    };
    let rich = rich.color(ui.visuals().hyperlink_color);
    let shown = glowfly_core::text::printable(url);
    if glowfly_core::forge::https_host(url).is_some() {
        let r = ui.add(egui::Link::new(rich)).on_hover_text(shown).on_hover_cursor(CursorIcon::PointingHand);
        if r.clicked() {
            msgs.push(Msg::OpenUrl(url.clone()));
        }
    } else {
        ui.add(egui::Label::new(rich).selectable(false)).on_hover_text(format!("{shown}\n{ONLY_HTTPS}"));
    }
}

#[cfg(test)]
#[path = "markdown_inline_tests.rs"]
mod tests;
