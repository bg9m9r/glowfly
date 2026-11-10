//! Headless tests for the palette's Delete tag… entry: listed only while the repository has
//! tags, its step lists the tags with their commits' short shas, and a chosen tag opens the
//! Delete tag dialog.

use crate::flows::branch_tests::with_origin;
use crate::flows::update;
use crate::flows::{drive, git};
use glowfly_model::msg::{Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::palette::{Mode, PaletteCommand as C, Pick, PickKind, shown};
use glowfly_model::state::{AppState, Modal};
use std::time::Instant;

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn listed(st: &AppState) -> bool {
    shown(st, "").into_iter().any(|s| s.entry.label == "Delete tag…")
}

fn run(st: &mut AppState, cmd: C) {
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now());
}

#[test]
fn delete_tag_step_opens_the_dialog() {
    let (mut st, work, bare) = with_origin("pal-delete-tag");
    assert!(!listed(&st), "no tags: no entry");
    run(&mut st, C::ChooseTagToDelete);
    assert!(st.palette.is_none(), "no tags: no step");

    git(&work, &["tag", "v1"]);
    git(&work, &["tag", "-a", "v2", "-m", "Two"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(listed(&st));
    run(&mut st, C::ChooseTagToDelete);
    let pk = pick(&st);
    assert_eq!(pk.kind, PickKind::DeleteTag);
    // In the snapshot's order, each with its commit's short sha.
    let tags = st.snap.as_ref().unwrap().tags.clone();
    assert_eq!(tags.len(), 2);
    for (i, (name, oid)) in tags.iter().enumerate() {
        assert_eq!(pk.label(i), name);
        assert_eq!(pk.detail(i), Some(oid.to_string()[..7].to_owned()));
        assert_eq!(pk.command(i), Some(C::DeleteTag(name.clone())));
    }
    let k = PickKind::DeleteTag;
    assert_eq!((k.title(), k.prompt(), k.loading()), ("Delete tag", "Find a tag to delete", "Loading tags…"));
    assert_eq!((k.none_text(), k.empty_text()), ("No matching tags", "This repository has no tags"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to choose, Backspace or Esc to go back");
    assert_eq!(k.more_text(1, 2), "Showing 1 of 2 tags. Type more to narrow the list.");
    assert!(k.lists_refs() && !k.dims_folders());

    run(&mut st, C::DeleteTag("v1".into()));
    assert!(matches!(&st.modal, Some(Modal::DeleteTag(f)) if f.tag == "v1"), "the dialog opens for the tag");
    assert!(st.palette.is_none(), "the palette closes for the dialog");
    assert_eq!(C::ChooseTagToDelete.recall_key(), Some("delete-tag"));
    assert_eq!(C::DeleteTag("v1".into()).recall_key(), None);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}
