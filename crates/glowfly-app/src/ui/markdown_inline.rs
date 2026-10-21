//! Markdown text runs (`ui/markdown.rs`): one label per run in its style, wrapped as one
//! paragraph. Links show their address on hover; only an https link opens (through
//! `Msg::OpenUrl`), since a relative or other link in a description has nowhere safe to go.

use super::theme::{color, type_scale};
use crate::msg::Msg;
use eframe::egui::{self, Color32, CursorIcon, RichText, Ui};
use glowfly_core::markdown::Inline;

pub(in crate::ui) const ONLY_HTTPS: &str = "Only https links open in the browser";

/// How a block draws its runs.
#[derive(Clone, Copy)]
pub(in crate::ui) struct Look {
    /// A type step.
    pub size: f32,
    /// Every run SemiBold (a heading).
    pub strong: bool,
    pub color: Color32,
}

/// Lays `runs` out as one paragraph wrapped to the width (`wrap`), or on one line; a hard
/// break starts a new line.
pub(in crate::ui) fn runs(ui: &mut Ui, runs: &[Inline], look: Look, wrap: bool, msgs: &mut Vec<Msg>) {
    let add = |ui: &mut Ui, msgs: &mut Vec<Msg>| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for run in runs {
            for (i, part) in run.text.split('\n').enumerate() {
                if i > 0 {
                    ui.end_row();
                }
                if !part.is_empty() {
                    one(ui, run, part, look, msgs);
                }
            }
        }
    };
    if wrap {
        ui.horizontal_wrapped(|ui| add(ui, msgs));
    } else {
        ui.horizontal(|ui| add(ui, msgs));
    }
}

fn one(ui: &mut Ui, run: &Inline, text: &str, look: Look, msgs: &mut Vec<Msg>) {
    let s = run.style;
    let strong = look.strong || s.strong;
    let font = if s.code {
        type_scale::mono(look.size)
    } else if strong {
        type_scale::strong(ui, look.size)
    } else {
        type_scale::font(look.size)
    };
    let mut rich = RichText::new(text).font(font);
    if s.code {
        rich = rich.background_color(color::raised());
    }
    if s.emphasis || s.image {
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
mod tests {
    use super::super::markdown_cache::MarkdownCache;
    use crate::msg::Msg;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn an_https_link_opens_in_the_browser_and_others_do_nothing() {
        let mut cache = MarkdownCache::default();
        let src = "[docs](https://example.com/docs) and [local](a.md)";
        let mut h =
            Harness::new_ui_state(move |ui, msgs: &mut Vec<Msg>| super::super::markdown::body(ui, "t", src, &mut cache, msgs), Vec::new());
        h.run();
        h.get_by_label("docs").click();
        h.run();
        assert!(matches!(h.state().as_slice(), [Msg::OpenUrl(u)] if u == "https://example.com/docs"));
        h.state_mut().clear();
        h.get_by_label("local").click();
        h.run();
        assert!(h.state().is_empty());
    }
}
