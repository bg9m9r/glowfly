//! Views take `&AppState` and the `&mut ViewState` that `AppState::with_view` lends them, so the
//! compiler keeps them from changing the model (#370): a model field a view writes is drawn
//! from a copy and sent back as `Msg::Edit` (a dialog's form whole, Preferences' fields, the
//! palette's query, the panels' drafts), and the secrets stay in `ViewState::secrets`. No view
//! takes `&mut AppState`, and none reaches `st.view`: while it is lent that field is a
//! placeholder (reaching it panics in debug builds), so a view uses the `ViewState` it is given.

use crate::lexer::bare_lines;
use crate::views::view_sources;

/// Each line of glowfly-ui's non-test sources where `hit` finds something, for the report.
fn lines_with(hit: impl Fn(&str) -> bool) -> Vec<String> {
    let mut found = Vec::new();
    for (rel, text) in view_sources() {
        for (i, line) in bare_lines(&text).iter().enumerate() {
            if hit(line) {
                found.push(format!("crates/glowfly-ui/src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    found
}

/// Whether `line` reaches the state's own `view` field (`st.view` or `state.view`), which is
/// lent out while the views draw.
fn reaches_lent_view(line: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    ["st.view", "state.view"].iter().any(|name| {
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            let after = line[at + name.len()..].chars().next();
            !before.is_some_and(|c| ident(c) || c == '.') && !after.is_some_and(ident)
        })
    })
}

#[test]
fn no_view_takes_the_whole_state() {
    let found = lines_with(|line| line.contains("&mut AppState"));
    assert!(
        found.is_empty(),
        "a view took `&mut AppState`: give it `st: &AppState, view: &mut ViewState` (lent by \
         `AppState::with_view`), draw model fields from a copy and push `Msg::Edit` (or another Msg) \
         for anything it changes:\n{}",
        found.join("\n")
    );
}

#[test]
fn views_never_reach_the_lent_view_state() {
    let found = lines_with(reaches_lent_view);
    assert!(
        found.is_empty(),
        "a view reached `st.view`, which is lent out while the views draw: use the `view: &mut \
         ViewState` it was given:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_lent_view_matcher_finds_the_field_only() {
    for hit in ["if st.view.show_search {", "let v = &mut state.view;", "f(st.view)"] {
        assert!(reaches_lent_view(hit), "{hit}");
    }
    for miss in ["view.show_search = true;", "st.views.len()", "st.viewport", "first.view.x", "cst.view.x", "app.st.with_view(f)"] {
        assert!(!reaches_lent_view(miss), "{miss}");
    }
}
