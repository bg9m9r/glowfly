//! Headless tests for the palette's Compare with working directory: when it is offered (one graph
//! commit selected), that it starts the comparison, its recent key, and the Create branch and
//! tag labels beside it.

use super::selection_tests::{fixture, up};
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, RepoMsg, SelectionMsg};
use crate::state::palette::{PaletteCommand as C, entries, shown};
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection};
use git2::Oid;

fn label(oid: Oid) -> String {
    format!("Compare {} with working directory", &oid.to_string()[..7])
}

fn compare_entries(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).filter(|l| l.starts_with("Compare ")).collect()
}

/// Opens the palette, closing an open one first.
fn reopen(st: &mut AppState) {
    st.palette = None;
    st.modal = None;
    up(st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_some());
}

#[test]
fn offered_for_one_selected_graph_commit() {
    let (mut st, f) = fixture("palette-compare-listed");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    assert_eq!(compare_entries(&st), [label(f.c(3))]);
    let e = entries(&st).into_iter().find(|e| e.label == label(f.c(3))).unwrap();
    assert_eq!(e.command, C::CompareWithWorkdir(f.c(3)));
    assert!(e.keywords.contains("working tree") && e.keywords.contains("uncommitted"));

    let not = [
        Selection::Wip,
        Selection::None,
        Selection::VsWorkdir(WorkdirCompare::new(f.c(3))),
        // A stash selected in the sidebar is not a graph row.
        Selection::Commit(Oid::from_str("1234567890123456789012345678901234567890").unwrap()),
    ];
    for sel in not {
        st.selection = sel.clone();
        assert!(compare_entries(&st).is_empty(), "{sel:?}");
    }
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    assert!(matches!(st.selection, Selection::Commits(_)));
    assert!(compare_entries(&st).is_empty(), "several commits");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn running_it_compares_and_is_recalled_for_the_selected_commit() {
    let (mut st, f) = fixture("palette-compare-run");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    reopen(&mut st);
    let fx = up(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::CompareWithWorkdir(f.c(3)))));
    assert!(st.palette.is_none(), "the palette closes");
    assert!(matches!(&st.selection, Selection::VsWorkdir(w) if w.oid == f.c(3)), "{:?}", st.selection);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadWorkdirCompare { oid, swapped: false, .. } if *oid == f.c(3))), "{fx:?}");
    assert_eq!(st.palette_recent.keys(), ["compare-workdir"]);

    // Recalled, it names the commit selected now.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(2))));
    reopen(&mut st);
    assert_eq!(shown(&st, "")[0].entry.label, label(f.c(2)));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn create_branch_and_tag_name_where_they_start() {
    let (mut st, f) = fixture("palette-compare-at");
    let labels = |st: &AppState| -> Vec<String> {
        entries(st).into_iter().map(|e| e.label).filter(|l| l.starts_with("Create branch") || l.contains(" tag at ")).collect()
    };
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    let at = &f.c(3).to_string()[..7];
    assert_eq!(labels(&st), [format!("Create branch at {at}…"), format!("Create tag at {at}…"), format!("Create annotated tag at {at}…")]);
    st.selection = Selection::Wip;
    assert_eq!(labels(&st), ["Create branch at HEAD…", "Create tag at HEAD…", "Create annotated tag at HEAD…"]);
    let _ = std::fs::remove_dir_all(&f.dir);
}
