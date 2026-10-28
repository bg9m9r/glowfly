//! What the smoke probe (`smoke_probe.rs`) reports: the widgets of the pass being drawn with
//! their accessible names and rectangles, the panels, the window frame drawn, the keyboard focus
//! and a few `AppState` fields, as JSON for the Linux installer checks
//! (`scripts/installer-lib-linux.sh`). No I/O: it reads egui's current pass and the state only, and is
//! called by the shell at the end of a frame (AccessKit nodes exist only inside a pass).
//!
//! The probe forces AccessKit on (`enable`), so the names exist from the first frame;
//! `accesskit_node_builder` also adds an empty node for each unnamed widget it is asked about,
//! which only matters while probing.

use crate::state::window_chrome::Chrome;
use crate::state::{AppState, Selection};
use eframe::egui::{self, Id, Pos2, Rect, WidgetRect, containers::panel::PanelState};
use serde_json::{Value, json};

/// The panels the probe reports and regions are cut from, by their egui ids. The drawn title
/// bar (`title-bar`) replaces the menu bar and tabs on Windows and macOS.
const PANELS: [&str; 7] = ["title-bar", "toolbar", "refs", "detail", "tabs", "status", "activity"];

/// Turns AccessKit on for every later pass, so widgets carry their names. Called once by the
/// shell when the probe is on.
pub fn enable(ctx: &egui::Context) {
    ctx.enable_accesskit();
}

/// The probe's JSON (schema v1) for the pass being drawn.
pub fn snapshot(ctx: &egui::Context, st: &AppState) -> Value {
    let drawn = panels(ctx);
    let mut panel_map = serde_json::Map::new();
    for id in PANELS {
        let rect = drawn.iter().find(|(p, _)| *p == id).map(|(_, r)| rect_json(*r));
        panel_map.insert(id.into(), rect.unwrap_or(Value::Null));
    }
    let screen = ctx.screen_rect();
    let snap = st.snap.as_ref();
    let toasts: Vec<Value> = st.toasts.iter().map(|t| json!({ "ok": t.ok, "text": t.text })).collect();
    json!({
        "v": 1,
        "pixels_per_point": ctx.pixels_per_point(),
        "native_pixels_per_point": ctx.input(|i| i.viewport().native_pixels_per_point),
        "zoom_percent": st.settings.zoom_percent,
        "chrome": chrome(st.view.chrome),
        "screen": [screen.width(), screen.height()],
        "focused": st.view.focused,
        "busy": st.job.is_some() || st.loading,
        "repo": snap.map(|s| s.real_workdir.to_string_lossy().into_owned()),
        "head": snap.and_then(|s| s.head_oid).map(|o| o.to_string()),
        "head_branch": snap.and_then(|s| s.head_branch.clone()),
        "selection": selection(&st.session.selection),
        "draft_summary": st.session.draft.summary,
        "modal": st.modal.is_some(),
        "palette": st.palette.is_some(),
        "prefs": st.prefs.is_some(),
        "context_menu": ctx.is_context_menu_open(),
        "popup": ctx.memory(|m| m.any_popup_open()),
        "toasts": toasts,
        "panels": panel_map,
        "keyboard_focus": keyboard_focus(ctx),
        "widgets": widgets_in(ctx, &drawn),
    })
}

/// The selection as the script compares it.
fn selection(s: &Selection) -> String {
    match s {
        Selection::None => "none".into(),
        Selection::Wip => "wip".into(),
        Selection::Commit(oid) => format!("commit:{oid}"),
        Selection::Commits(m) => format!("commits:{}", m.len()),
        Selection::VsWorkdir(_) => "vs-workdir".into(),
    }
}

/// The window frame being drawn, as the scripts compare it.
fn chrome(c: Chrome) -> &'static str {
    match c {
        Chrome::System => "system",
        Chrome::Windows => "windows",
        Chrome::MacOs => "macos",
    }
}

fn rect_json(r: Rect) -> Value {
    json!([r.min.x, r.min.y, r.max.x, r.max.y])
}

/// The panels drawn in this pass, with their rectangles. egui keeps a hidden panel's last
/// rectangle in its memory, so a panel counts only when its own `Ui` was made this pass.
fn panels(ctx: &egui::Context) -> Vec<(&'static str, Rect)> {
    let drawn = ctx.viewport(|vp| PANELS.map(|id| vp.this_pass.widgets.get(Id::new(id)).is_some()));
    PANELS
        .into_iter()
        .zip(drawn)
        .filter(|(_, d)| *d)
        .filter_map(|(id, _)| PanelState::load(ctx, Id::new(id)).map(|p| (id, p.rect)))
        .collect()
}

/// Which part of the window `p` is in: the open palette, else the first panel holding it
/// (refs as "sidebar", detail as "details", title-bar as "title_bar"), else "central". Mirrors the journeys' regions
/// (`journey/find.rs`).
fn region_of(panels: &[(&str, Rect)], palette: Option<Rect>, p: Pos2) -> &'static str {
    if palette.is_some_and(|r| r.contains(p)) {
        return "palette";
    }
    let Some((id, _)) = panels.iter().find(|(_, r)| r.contains(p)) else { return "central" };
    match *id {
        "title-bar" => "title_bar",
        "refs" => "sidebar",
        "detail" => "details",
        "toolbar" => "toolbar",
        "tabs" => "tabs",
        "status" => "status",
        "activity" => "activity",
        _ => "central",
    }
}

/// A widget's AccessKit role, label, value and whether it is disabled.
type Node = (String, Option<String>, Option<String>, bool);

fn node(ctx: &egui::Context, id: Id) -> Option<Node> {
    ctx.accesskit_node_builder(id, |n| {
        (format!("{:?}", n.role()), n.label().map(str::to_owned), n.value().map(str::to_owned), n.is_disabled())
    })
}

/// The widget with the keyboard focus, by its name and role.
fn keyboard_focus(ctx: &egui::Context) -> Value {
    let Some(id) = ctx.memory(|m| m.focused()) else { return Value::Null };
    match node(ctx, id) {
        Some((role, label, value, _)) => json!({ "label": label.or(value), "role": role }),
        None => Value::Null,
    }
}

/// Every named widget of this pass on screen (`panels` from `panels`): its label, value, role,
/// whether it is enabled, its rectangle in points, its region and its layer's order.
fn widgets_in(ctx: &egui::Context, panels: &[(&str, Rect)]) -> Vec<Value> {
    // Copied out first: reading a node locks the context as `viewport` does.
    let rects: Vec<WidgetRect> = ctx.viewport(|vp| vp.this_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().copied()).collect());
    let palette = super::palette::area_rect(ctx);
    let mut out = Vec::new();
    for w in rects {
        let w = match ctx.layer_transform_to_global(w.layer_id) {
            Some(t) => w.transform(t),
            None => w,
        };
        if !w.interact_rect.is_positive() {
            continue;
        }
        let Some((role, label, value, disabled)) = node(ctx, w.id) else { continue };
        if label.is_none() && value.is_none() {
            continue;
        }
        out.push(json!({
            "label": label,
            "value": value,
            "role": role,
            "enabled": w.enabled && !disabled,
            "rect": rect_json(w.rect),
            "region": region_of(panels, palette, w.rect.center()),
            "layer": format!("{:?}", w.layer_id.order),
        }));
    }
    out
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
