//! Keeps every menu on the one menu style (`ui/widgets/menu.rs`): views open context menus,
//! submenus, dropdown menus and popups only through its wrappers (`menu::context_menu`,
//! `menu::submenu`, `menu::popup_below`, `menu::popup_above_or_below`, `menu::popover_below`),
//! `RowMenuExt::row_menu` or `menu_anchor::menu_below`, which apply the style before the
//! contents. egui's own openers would draw the dense egui rows, or a popup's filled buttons.
//!
//! Test code is skipped (`support/views.rs`).

#[path = "support/views.rs"]
mod views;

use views::{code_lines, view_sources};

/// The files that wrap egui's openers.
const WRAPPERS: &[&str] = &["ui/widgets/menu.rs", "ui/widgets/row_menu.rs", "ui/menu_anchor.rs"];

/// egui's openers, as (pattern, the wrapper to use instead).
const DENIED: &[(&str, &str)] = &[
    ("ui.menu_button(", "menu::submenu"),
    (".menu_image_button(", "menu::submenu"),
    (".context_menu(", "menu::context_menu or RowMenuExt::row_menu"),
    ("popup_below_widget(", "menu::popup_below (a menu) or menu::popover_below"),
    ("popup_above_or_below_widget(", "menu::popup_above_or_below"),
    (".bar_menu(", "menu_anchor::menu_below"),
    ("egui::menu::", "the wrappers in ui/widgets/menu.rs"),
];

/// Problems in one file's source, as "line: message".
fn violations(src: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        for (pat, instead) in DENIED {
            if line.contains(pat) {
                found.push(format!("{}: `{pat}`; use {instead}", i + 1));
            }
        }
    }
    found
}

#[test]
fn menus_open_only_through_the_shared_style() {
    let mut report = Vec::new();
    for (rel, text) in view_sources() {
        if WRAPPERS.contains(&rel.as_str()) {
            continue;
        }
        report.extend(violations(&text).into_iter().map(|v| format!("src/{rel}:{v}")));
    }
    assert!(report.is_empty(), "open menus through ui/widgets/menu.rs so they get the shared style:\n{}", report.join("\n"));
}

#[test]
fn the_check_catches_egui_openers() {
    let src = "fn a(ui: &mut Ui) {\n    ui.menu_button(\"Copy\", |ui| {});\n    resp.context_menu(|ui| {});\n    \
               egui::popup::popup_below_widget(ui, id, &r, close, |ui| {});\n    egui::menu::context_menu(&r, |ui| {});\n}\n";
    let found = violations(src);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].starts_with("2: `ui.menu_button(`"));
    let ok = "fn b(ui: &mut Ui) {\n    submenu(ui, \"Copy\", |ui| {});\n    context_menu(&resp, |ui| {});\n    \
              popup_below(ui, id, &r, close, |ui| {});\n    reason::menu_button(ui, \"Push\", \"\", s);\n    // ui.menu_button(\n}\n";
    assert!(violations(ok).is_empty(), "{:?}", violations(ok));
}
