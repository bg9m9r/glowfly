//! The `Icon` set and `paint_icon`: the first icons' shapes here, the newer ones in `ui/icons.rs`.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Stroke, pos2, vec2};

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Pull,
    Push,
    Branch,
    Stash,
    Pop,
    Terminal,
    Refresh,
    Search,
    PullRequest,
    Activity,
    Folder,
    Gear,
    User,
    Plug,
    Brush,
    Key,
    Hook,
    Back,
    Identity,
    Undo,
    Redo,
    /// Up and down arrows for previous / next change in the diff viewer.
    PrevChange,
    NextChange,
    /// Two columns of three dots: a drag handle.
    Grip,
    /// Angle brackets around a slash: code (Preferences > Editor).
    Code,
    /// Show / hide a masked field's text.
    Eye,
    EyeOff,
    /// A database cylinder: Git LFS storage (Preferences).
    Storage,
    /// Trunk with a side branch that leaves and rejoins: Git Flow (Preferences).
    Flow,
    /// A search field over two list rows: the command palette (toolbar).
    Palette,
    /// A shield with an exclamation mark: an untrusted repository (toolbar chip).
    Shield,
    /// A folder with one solid and one dotted file row: sparse checkout (Preferences).
    Sparse,
    /// A five-pointed star, outlined or filled: a favorite repository toggle.
    Star,
    StarFilled,
    /// A small diagonal cross: remove an entry from a list.
    Close,
    /// A padlock: SSH keys (Preferences).
    Lock,
    /// A wrench: external tools (Preferences).
    Tools,
    /// A triangle with an exclamation mark: predicted merge conflicts (toolbar chip).
    Warning,
    /// A keyboard: Preferences > Keyboard.
    Keyboard,
    /// Two pages side by side with an arrow out of the corner: open in the external diff tool.
    ExternalDiff,
    /// A small arrow pointing right: one branch merging into another (the pull request panel).
    Into,
    /// A speech bubble: code comment threads (pull request panel).
    Comment,
    /// A pencil: edit an entry (the font lacks a pencil glyph).
    Pencil,
    /// A cross of two strokes: add an item (the ISSUES header's Create issue).
    Plus,
    /// A circle with a dot in the middle: an issue (the issue panel).
    Issue,
    /// A tick of two strokes: a resolved file (the font lacks a check mark).
    Check,
    /// Three dots in a row: a menu of more actions (the font lacks the ellipsis glyph's weight).
    More,
}

/// Every icon's line width: they are outlines only.
pub const STROKE: f32 = 1.6;

pub fn paint_icon(p: &Painter, c: Pos2, icon: Icon, col: Color32) {
    let s = Stroke::new(STROKE, col);
    if crate::ui::icons::paint(p, c, icon, s) {
        return;
    }
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], s);
    let arrow_down = |top: f32, bottom: f32| {
        line(pos2(c.x, top), pos2(c.x, bottom));
        line(pos2(c.x - 5.0, bottom - 5.0), pos2(c.x, bottom));
        line(pos2(c.x + 5.0, bottom - 5.0), pos2(c.x, bottom));
    };
    let arrow_up = |top: f32, bottom: f32| {
        line(pos2(c.x, top), pos2(c.x, bottom));
        line(pos2(c.x - 5.0, top + 5.0), pos2(c.x, top));
        line(pos2(c.x + 5.0, top + 5.0), pos2(c.x, top));
    };
    let tray = || {
        line(pos2(c.x - 8.0, c.y + 1.0), pos2(c.x - 8.0, c.y + 8.0));
        line(pos2(c.x - 8.0, c.y + 8.0), pos2(c.x + 8.0, c.y + 8.0));
        line(pos2(c.x + 8.0, c.y + 8.0), pos2(c.x + 8.0, c.y + 1.0));
    };
    match icon {
        Icon::Pull => {
            arrow_down(c.y - 8.0, c.y + 4.0);
            line(pos2(c.x - 7.0, c.y + 8.0), pos2(c.x + 7.0, c.y + 8.0));
        }
        Icon::Push => {
            arrow_up(c.y - 6.0, c.y + 6.0);
            line(pos2(c.x - 7.0, c.y + 9.0), pos2(c.x + 7.0, c.y + 9.0));
        }
        Icon::PrevChange => arrow_up(c.y - 6.0, c.y + 6.0),
        Icon::NextChange => arrow_down(c.y - 6.0, c.y + 6.0),
        Icon::Grip => {
            for dx in [-2.5, 2.5] {
                for dy in [-5.0, 0.0, 5.0] {
                    p.circle_filled(pos2(c.x + dx, c.y + dy), 1.4, col);
                }
            }
        }
        Icon::Branch => {
            p.circle_stroke(pos2(c.x - 4.0, c.y - 7.0), 2.4, s);
            p.circle_stroke(pos2(c.x - 4.0, c.y + 7.0), 2.4, s);
            p.circle_stroke(pos2(c.x + 5.0, c.y - 3.0), 2.4, s);
            line(pos2(c.x - 4.0, c.y - 4.6), pos2(c.x - 4.0, c.y + 4.6));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [pos2(c.x + 5.0, c.y - 0.6), pos2(c.x + 5.0, c.y + 3.0), pos2(c.x - 4.0, c.y + 1.0), pos2(c.x - 4.0, c.y + 4.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Stash => {
            tray();
            arrow_down(c.y - 9.0, c.y + 4.0);
        }
        Icon::Pop => {
            tray();
            arrow_up(c.y - 9.0, c.y + 4.0);
        }
        Icon::Terminal => {
            line(pos2(c.x - 8.0, c.y - 5.0), pos2(c.x - 2.0, c.y));
            line(pos2(c.x - 2.0, c.y), pos2(c.x - 8.0, c.y + 5.0));
            line(pos2(c.x + 1.0, c.y + 6.0), pos2(c.x + 8.0, c.y + 6.0));
        }
        // Clockwise arc with a gap at the top right.
        Icon::Refresh => arc_arrow(p, c, -0.9, 5.0, s),
        // Counter-clockwise over the top, ending at the left; Redo is its mirror image.
        Icon::Undo => arc_arrow(p, c + vec2(0.0, 1.5), -0.3, -3.6, s),
        Icon::Redo => arc_arrow(p, c + vec2(0.0, 1.5), std::f32::consts::PI + 0.3, 3.6, s),
        Icon::Search => {
            p.circle_stroke(pos2(c.x - 2.0, c.y - 2.0), 5.5, s);
            line(pos2(c.x + 2.0, c.y + 2.0), pos2(c.x + 8.0, c.y + 8.0));
        }
        Icon::PullRequest => {
            p.circle_stroke(pos2(c.x - 5.0, c.y - 7.0), 2.4, s);
            p.circle_stroke(pos2(c.x - 5.0, c.y + 7.0), 2.4, s);
            p.circle_stroke(pos2(c.x + 5.0, c.y + 7.0), 2.4, s);
            line(pos2(c.x - 5.0, c.y - 4.6), pos2(c.x - 5.0, c.y + 4.6));
            line(pos2(c.x + 5.0, c.y + 4.6), pos2(c.x + 5.0, c.y - 5.0));
            line(pos2(c.x + 5.0, c.y - 5.0), pos2(c.x + 1.0, c.y - 5.0));
            line(pos2(c.x + 1.0, c.y - 5.0), pos2(c.x + 3.0, c.y - 7.5));
            line(pos2(c.x + 1.0, c.y - 5.0), pos2(c.x + 3.0, c.y - 2.5));
        }
        // A list: a short and a long line on each of three rows.
        Icon::Activity => {
            for dy in [-6.0, 0.0, 6.0] {
                line(pos2(c.x - 8.0, c.y + dy), pos2(c.x - 6.0, c.y + dy));
                line(pos2(c.x - 3.0, c.y + dy), pos2(c.x + 8.0, c.y + dy));
            }
        }
        Icon::Gear => {
            p.circle_stroke(c, 3.0, s);
            p.circle_stroke(c, 6.5, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                line(pos2(c.x + 6.5 * a.cos(), c.y + 6.5 * a.sin()), pos2(c.x + 9.0 * a.cos(), c.y + 9.0 * a.sin()));
            }
        }
        Icon::User | Icon::Identity => {
            p.circle_stroke(pos2(c.x, c.y - 4.0), 3.5, s);
            p.add(egui::Shape::line(
                (0..=12)
                    .map(|i| {
                        let a = std::f32::consts::PI + i as f32 / 12.0 * std::f32::consts::PI;
                        pos2(c.x + 7.0 * a.cos(), c.y + 8.0 + 6.0 * a.sin())
                    })
                    .collect(),
                s,
            ));
            if icon == Icon::Identity {
                line(pos2(c.x + 5.0, c.y - 8.0), pos2(c.x + 9.0, c.y - 8.0));
                line(pos2(c.x + 5.0, c.y - 5.0), pos2(c.x + 9.0, c.y - 5.0));
            }
        }
        Icon::Plug => {
            line(pos2(c.x - 3.0, c.y - 9.0), pos2(c.x - 3.0, c.y - 4.0));
            line(pos2(c.x + 3.0, c.y - 9.0), pos2(c.x + 3.0, c.y - 4.0));
            p.add(egui::Shape::closed_line(
                vec![pos2(c.x - 6.0, c.y - 4.0), pos2(c.x + 6.0, c.y - 4.0), pos2(c.x + 4.0, c.y + 3.0), pos2(c.x - 4.0, c.y + 3.0)],
                s,
            ));
            line(pos2(c.x, c.y + 3.0), pos2(c.x, c.y + 9.0));
        }
        Icon::Brush => {
            line(pos2(c.x + 8.0, c.y - 8.0), pos2(c.x - 1.0, c.y + 1.0));
            p.circle_filled(pos2(c.x - 4.0, c.y + 4.0), 4.0, col);
        }
        Icon::Key => {
            p.circle_stroke(pos2(c.x - 4.0, c.y), 4.0, s);
            line(pos2(c.x, c.y), pos2(c.x + 9.0, c.y));
            line(pos2(c.x + 6.0, c.y), pos2(c.x + 6.0, c.y + 4.0));
            line(pos2(c.x + 9.0, c.y), pos2(c.x + 9.0, c.y + 3.0));
        }
        Icon::Hook => {
            p.text(c, Align2::CENTER_CENTER, "{ }", FontId::monospace(13.0), col);
        }
        Icon::Back => {
            line(pos2(c.x + 7.0, c.y), pos2(c.x - 7.0, c.y));
            line(pos2(c.x - 7.0, c.y), pos2(c.x - 2.0, c.y - 5.0));
            line(pos2(c.x - 7.0, c.y), pos2(c.x - 2.0, c.y + 5.0));
        }
        Icon::Folder => {
            let pts = vec![
                pos2(c.x - 9.0, c.y + 7.0),
                pos2(c.x - 9.0, c.y - 7.0),
                pos2(c.x - 3.0, c.y - 7.0),
                pos2(c.x - 1.0, c.y - 4.0),
                pos2(c.x + 9.0, c.y - 4.0),
                pos2(c.x + 9.0, c.y + 7.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Code | Icon::Eye | Icon::EyeOff | Icon::Storage | Icon::Flow | Icon::Palette | Icon::Shield | Icon::Sparse => {} // icons.rs
        Icon::Star | Icon::StarFilled | Icon::Close | Icon::Lock | Icon::Tools | Icon::Warning | Icon::Keyboard => {}          // icons.rs
        Icon::ExternalDiff | Icon::Into | Icon::Comment | Icon::Pencil | Icon::Plus | Icon::Issue | Icon::Check | Icon::More => {} // icons.rs
    }
}

/// An arc of radius 7 around `c` from angle `start` (radians, clockwise from +x) through `sweep`,
/// with an open arrowhead of two strokes at its end pointing along the direction of travel.
fn arc_arrow(p: &Painter, c: Pos2, start: f32, sweep: f32, s: Stroke) {
    const R: f32 = 7.0;
    const HEAD: f32 = 5.0; // the barbs' length back along the arc
    let at = |a: f32| pos2(c.x + R * a.cos(), c.y + R * a.sin());
    let end = start + sweep;
    // The head points along the chord across the arc's last HEAD px: the tangent at the end
    // alone aims it off the curve, since a 7 px circle bends away within that length.
    let (tip, base) = (at(end), at(end - sweep.signum() * HEAD / R));
    let dir = (tip - base).normalized();
    let normal = vec2(dir.y, -dir.x);
    let points = (0..=24).map(|i| at(start + i as f32 / 24.0 * sweep)).collect();
    p.add(egui::Shape::line(points, s));
    let back = tip - dir * 4.0;
    p.line_segment([back + normal * 3.5, tip], s);
    p.line_segment([back - normal * 3.5, tip], s);
}
