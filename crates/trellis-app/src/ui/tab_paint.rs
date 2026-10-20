//! How a repository tab looks (`tabs.rs` lays the strip out): the repository icon, the name and
//! the close ×, in a shape with rounded top corners. A quiet tab has no fill (the chrome shows
//! through), a hovered one the hover fill and a pressed one the pressed fill. The active tab
//! stands 2 px taller on the toolbar's own fill, so it runs into the toolbar below, with one
//! 2 px band in its checked-out branch's colour along its rounded top. The × fades in on the
//! active and hovered tabs without ever changing the layout. Neighbouring quiet tabs are parted
//! by a short line, and a worktree group's tabs share a strip along their bottom.

use super::theme::color;
use super::widgets::tab_glyph::{self, TabGlyph};
use super::widgets::{Cut, Icon, paint_cut_text, paint_icon_sized};
use eframe::egui::{self, Color32, FontId, Rect, Stroke, StrokeKind, pos2, vec2};

/// A tab's corners: rounded at the top, square where it meets the bar's bottom.
pub(super) const ROUNDING: egui::CornerRadius = egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
/// How much taller than its slot the active (and the dragged) tab stands.
pub(super) const LIFT: f32 = 2.0;
/// The band along the active tab's top.
const BAND: f32 = 2.0;
/// A tab's room besides its name: the padding, the icon and its gap, the gap before the close
/// slot, the close slot and the padding after it.
const PAD_L: f32 = 12.0;
const ICON: f32 = 14.0;
const ICON_GAP: f32 = 8.0;
const CLOSE_GAP: f32 = 8.0;
const PAD_R: f32 = 10.0;
pub(super) const CHROME_W: f32 = PAD_L + ICON + ICON_GAP + CLOSE_GAP + tab_glyph::BOX + PAD_R;

/// What a tab shows this frame.
#[derive(Clone, Copy)]
pub(super) struct TabLook {
    pub active: bool,
    pub hovered: bool,
    pub pressed: bool,
    /// Floating above the strip while dragged: lifted onto `fill` with a border.
    pub floating: bool,
    /// How visible the close × is, 0 to 1.
    pub close_alpha: f32,
    pub close_hovered: bool,
    /// The toolbar's fill, which the active and the dragged tab stand on.
    pub fill: Color32,
    /// The active tab's band: its checked-out branch's line tone.
    pub band: Color32,
}

impl TabLook {
    /// A tab that is neither active, hovered nor pressed, on `fill`.
    pub fn quiet(fill: Color32) -> Self {
        Self {
            active: false,
            hovered: false,
            pressed: false,
            floating: false,
            close_alpha: 0.0,
            close_hovered: false,
            fill,
            band: color::accent(),
        }
    }
}

/// Paints one tab in its slot `rect`; returns where its name went and whether it was cut.
pub(super) fn paint_tab(p: &egui::Painter, rect: Rect, name: &str, font: FontId, look: TabLook) -> Cut {
    let close_rect = close_rect(rect);
    let lifted = rect.with_min_y(rect.top() - LIFT);
    if look.active || look.floating {
        p.rect_filled(lifted, ROUNDING, look.fill);
    } else if look.pressed {
        p.rect_filled(rect, ROUNDING, color::selected());
    } else if look.hovered {
        p.rect_filled(rect, ROUNDING, color::hover());
    }
    if look.active {
        paint_band(p, lifted, look.band);
    }
    if look.floating {
        p.rect_stroke(lifted, ROUNDING, Stroke::new(1.0_f32, color::border()), StrokeKind::Inside);
    }
    let icon_tone = if look.active {
        color::accent()
    } else if look.hovered {
        color::weak()
    } else {
        color::faint()
    };
    let cy = rect.center().y;
    paint_icon_sized(p, pos2(rect.left() + PAD_L + ICON / 2.0, cy), Icon::Repo, icon_tone, ICON);
    let text_col = if look.active { color::text() } else { color::weak() };
    // The name, cut with an ellipsis before the close slot (kept whether or not the × shows).
    let left = rect.left() + PAD_L + ICON + ICON_GAP;
    let cut = paint_cut_text(p, pos2(left, cy), name, font, text_col, close_rect.left() - CLOSE_GAP - left);
    if look.close_alpha > 0.0 {
        let hover = look.close_hovered.then(|| color::selected().gamma_multiply(look.close_alpha));
        tab_glyph::paint(p, close_rect.center(), TabGlyph::Close, text_col.gamma_multiply(look.close_alpha), hover);
    }
    cut
}

/// The active tab's band: a `BAND` px line of `band` along the top of `rect`, curving down its
/// rounded corners to where they meet the sides.
fn paint_band(p: &egui::Painter, rect: Rect, band: Color32) {
    // The line's centre runs half its width inside the edge, on arcs of the corner's radius less that.
    let r = f32::from(ROUNDING.nw);
    let arc = r - BAND / 2.0;
    let (lc, rc) = (pos2(rect.left() + r, rect.top() + r), pos2(rect.right() - r, rect.top() + r));
    let steps = 8;
    let quarter = |c: egui::Pos2, from: f32| {
        (0..=steps).map(move |i| {
            let a = from + i as f32 / steps as f32 * std::f32::consts::FRAC_PI_2;
            pos2(c.x + arc * a.cos(), c.y + arc * a.sin())
        })
    };
    // Left corner from the side up to the top, then the right corner from the top down.
    let mut pts: Vec<egui::Pos2> = quarter(lc, std::f32::consts::PI).collect();
    pts.extend(quarter(rc, -std::f32::consts::FRAC_PI_2));
    p.add(egui::Shape::line(pts, Stroke::new(BAND, band)));
}

/// A tab's width for a name `text_w` wide (in the active tab's font, so a tab never changes
/// width when it is selected): its chrome around the name, between 96 and 220.
pub(super) fn tab_width(text_w: f32) -> f32 {
    (text_w + CHROME_W).clamp(96.0, 220.0)
}

/// The short line between two tabs that are neither active nor hovered, `gap` wide after `left`.
pub(super) fn paint_separator(p: &egui::Painter, left: Rect, gap: f32) {
    let x = left.right() + gap / 2.0;
    let y = left.center().y;
    p.line_segment([pos2(x, y - 8.0), pos2(x, y + 8.0)], Stroke::new(1.0_f32, color::border()));
}

/// The color of the `n`th group from the left, so neighbouring groups differ.
pub(super) fn group_color(n: usize) -> Color32 {
    color::repo_group(n).gamma_multiply(0.8)
}

/// The strip under a grouped tab: inset at the group's outer ends, and across the `gap` to the
/// next tab elsewhere, so it runs unbroken along the group.
pub(super) fn paint_group_strip(p: &egui::Painter, rect: Rect, color: Color32, first: bool, last: bool, gap: f32) {
    let left = rect.left() + if first { 4.0 } else { 0.0 };
    let right = rect.right() + if last { -4.0 } else { gap };
    p.rect_filled(Rect::from_min_max(pos2(left, rect.bottom() - 3.0), pos2(right, rect.bottom() - 1.0)), 1.0, color);
}

/// Where a tab's close × sits: its box `PAD_R` from the tab's right edge.
pub(super) fn close_rect(tab: Rect) -> Rect {
    let x = tab.right() - PAD_R - tab_glyph::BOX / 2.0;
    Rect::from_center_size(pos2(x, tab.center().y), vec2(tab_glyph::BOX, tab_glyph::BOX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_is_its_name_and_its_chrome() {
        assert_eq!(CHROME_W, 68.0);
        assert_eq!(tab_width(100.0), 168.0);
        assert_eq!(tab_width(10.0), 96.0, "the narrowest");
        assert_eq!(tab_width(400.0), 220.0, "the widest");
    }

    #[test]
    fn the_close_box_keeps_its_padding() {
        let tab = Rect::from_min_size(pos2(0.0, 0.0), vec2(160.0, 34.0));
        let close = close_rect(tab);
        assert_eq!((close.left(), close.right()), (160.0 - 26.0, 150.0));
        assert_eq!(close.center().y, 17.0);
    }
}
