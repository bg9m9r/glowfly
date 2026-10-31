//! A view's text snapshot: what the last frame shows, as text. It lists the accessibility tree
//! (one line per widget: its role, label, value and states, in the order egui made them) and
//! then the painted text no widget carries (graph rows, chips, painted headers). With a dialog
//! open it holds only the dialogs (their accessibility subtrees and the text painted inside
//! them), so a change to the window behind doesn't touch a dialog's snapshot. It leaves out
//! positions and sizes: a spacing or font change shouldn't rewrite hundreds of files; the
//! pixel goldens (`goldens.rs`) cover the look.

use eframe::egui::{self, Rect, Shape, accesskit::Role, epaint::ClippedShape};
use egui_kittest::Harness;
use egui_kittest::kittest::{Node, Queryable, by};
use std::collections::BTreeSet;

/// The text snapshot of `h`'s last frame.
pub(super) fn of<S>(h: &Harness<'_, S>) -> String {
    let screen = h.ctx.screen_rect();
    let root = h.kittest_state().root();
    let dialogs: Vec<Node<'_>> = kids(&root).into_iter().filter(|n| n.role() == Role::Dialog && !n.is_hidden()).collect();
    let mut out = Snapshot::default();
    if dialogs.is_empty() {
        out.line(format!("window {}x{}", screen.width().round(), screen.height().round()));
        for child in kids(&root) {
            out.node(&child, 0);
        }
    } else {
        for d in &dialogs {
            out.node(d, 0);
        }
    }
    let rects: Vec<Rect> = dialogs.iter().filter_map(bounds).collect();
    let painted = painted_text(&h.output().shapes, screen, (!rects.is_empty()).then_some(rects.as_slice()));
    let extra: Vec<String> = painted.into_iter().filter(|t| !out.said.contains(t)).collect();
    if !extra.is_empty() {
        out.line("painted:".into());
        for t in extra {
            out.line(format!("  {t:?}"));
        }
    }
    normalize(&out.text)
}

/// Where the open dialogs are in `h`'s last frame.
pub(super) fn dialog_rects<S>(h: &Harness<'_, S>) -> Vec<Rect> {
    let root = h.kittest_state().root();
    kids(&root).iter().filter(|n| n.role() == Role::Dialog && !n.is_hidden()).filter_map(bounds).collect()
}

/// The direct children of `n`.
fn kids<'t>(n: &Node<'t>) -> Vec<Node<'t>> {
    n.query_all(by().recursive(false)).collect()
}

fn bounds(n: &Node<'_>) -> Option<Rect> {
    n.bounding_box().map(|b| Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32)))
}

#[derive(Default)]
struct Snapshot {
    text: String,
    /// Every label and value listed, so the painted text leaves them out.
    said: BTreeSet<String>,
}

impl Snapshot {
    fn line(&mut self, s: String) {
        self.text.push_str(&s);
        self.text.push('\n');
    }

    /// `n` and its children, `depth` levels in. A node with nothing to say (egui's plain
    /// interaction areas: no label, value or state) is left out and its children move up.
    fn node(&mut self, n: &Node<'_>, depth: usize) {
        if n.is_hidden() || n.role() == Role::TextRun {
            return;
        }
        let mut label = n.label().filter(|s| !s.is_empty());
        let mut value = n.value().filter(|s| !s.is_empty() && Some(s) != label.as_ref());
        // egui gives a label's text as its value.
        if n.role() == Role::Label && label.is_none() {
            label = value.take();
        }
        let mut flags = Vec::new();
        if n.is_disabled() {
            flags.push("disabled");
        }
        match n.toggled() {
            Some(egui::accesskit::Toggled::True) => flags.push("on"),
            Some(egui::accesskit::Toggled::False) => flags.push("off"),
            Some(egui::accesskit::Toggled::Mixed) => flags.push("mixed"),
            None => {}
        }
        if n.is_selected() == Some(true) {
            flags.push("selected");
        }
        if n.is_focused() {
            flags.push("focused");
        }
        let numeric = n.numeric_value();
        let plain = matches!(n.role(), Role::Unknown | Role::GenericContainer);
        let silent = plain && label.is_none() && value.is_none() && flags.is_empty() && numeric.is_none();
        let depth = if silent {
            depth
        } else {
            let mut s = format!("{}{:?}", "  ".repeat(depth), n.role());
            if let Some(l) = &label {
                s.push_str(&format!(" {l:?}"));
                self.said.insert(l.trim().to_owned());
            }
            if let Some(v) = &value {
                s.push_str(&format!(" = {v:?}"));
                self.said.insert(v.trim().to_owned());
            }
            if let Some(x) = numeric {
                s.push_str(&format!(" = {x}"));
            }
            if !flags.is_empty() {
                s.push_str(&format!(" [{}]", flags.join(", ")));
            }
            self.line(s);
            depth + 1
        };
        for child in kids(n) {
            self.node(&child, depth);
        }
    }
}

/// The text painted on screen, in paint order. With `dialogs`, only what is painted inside them
/// over the first modal's backdrop (a see-through fill over the whole window).
fn painted_text(shapes: &[ClippedShape], screen: Rect, dialogs: Option<&[Rect]>) -> Vec<String> {
    let mut flat = Vec::new();
    for s in shapes {
        flatten(&s.shape, s.clip_rect, &mut flat);
    }
    let start = match dialogs {
        Some(_) => flat.iter().position(|(_, s)| is_backdrop(s, screen)).map_or(flat.len(), |i| i + 1),
        None => 0,
    };
    let mut out = Vec::new();
    for (clip, shape) in &flat[start..] {
        let Shape::Text(t) = shape else { continue };
        let rect = t.galley.rect.translate(t.pos.to_vec2());
        let shown = rect.intersect(*clip).intersect(screen);
        if !shown.is_positive() {
            continue;
        }
        if dialogs.is_some_and(|ds| !ds.iter().any(|d| d.contains(rect.center()))) {
            continue;
        }
        let text = t.galley.text().trim();
        if !text.is_empty() {
            out.push(text.to_owned());
        }
    }
    out
}

fn flatten<'a>(shape: &'a Shape, clip: Rect, out: &mut Vec<(Rect, &'a Shape)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| flatten(s, clip, out)),
        s => out.push((clip, s)),
    }
}

/// Whether `s` is a modal's backdrop: a see-through fill covering the window.
fn is_backdrop(s: &Shape, screen: Rect) -> bool {
    matches!(s, Shape::Rect(r) if r.rect.contains_rect(screen) && r.fill.a() > 0 && r.fill.a() < 255)
}

/// `s` without what changes from run to run: the temp folder and the process id in the
/// fixtures' paths.
fn normalize(s: &str) -> String {
    let temp = glowfly_core::paths::temp_dir();
    let temp = temp.to_string_lossy();
    let temp = temp.trim_end_matches(['\\', '/']);
    let mut out = s.to_owned();
    for t in [temp.to_owned(), temp.replace('\\', "/"), temp.replace('\\', "\\\\")] {
        if !t.is_empty() {
            out = out.replace(&t, "<temp>");
        }
    }
    out.replace(&format!("-{}-", std::process::id()), "-<pid>-")
}
