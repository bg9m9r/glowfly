//! Headless tests for one file of a commit saved as a patch, a commit's patch copied to the
//! clipboard, and the palette's patch entries.

use super::super::patch_tests::{cleanup, commit, last_log, last_toast, run, three_commits};
use super::super::tests::{drive, finish_keeping, git};
use super::super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::AppState;
use crate::state::Selection;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::patch::PATCH_BLOCKED_MERGE;
use crate::state::selection::MultiSelection;
use git2::Oid;
use std::time::Instant;
use trellis_core::patch_ops::{ExportReport, PatchSource, WipScope};

/// `three_commits` plus a side branch merged into main as "Merge side".
fn with_merge(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = three_commits(name);
    git(&dir, &["checkout", "-q", "-b", "side", "HEAD~1"]);
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "side"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

#[test]
fn one_file_of_a_commit_asks_where_to_save() {
    let (mut st, dir) = with_merge("patch-file-export");
    let third = commit(&st, "Third: more (lines)");
    let fx = run(&mut st, PatchMsg::ExportFiles { oid: third, paths: vec!["b.txt".into()] });
    match &fx[..] {
        [Effect::Patch(PatchEffect::PickSave { what: PatchSource::Commits { oids, paths }, name, .. })] => {
            assert_eq!((oids.as_slice(), paths.as_slice()), (&[third][..], &["b.txt".to_owned()][..]));
            assert_eq!(name, "Third-more-lines-b-txt.patch");
        }
        other => panic!("unexpected effects {other:?}"),
    }
    let merge = commit(&st, "Merge side");
    assert!(run(&mut st, PatchMsg::ExportFiles { oid: merge, paths: vec!["c.txt".into()] }).is_empty());
    assert_eq!(last_log(&st), PATCH_BLOCKED_MERGE);
    cleanup(&dir);
}

#[test]
fn copy_puts_the_patch_on_the_clipboard() {
    let (mut st, dir) = three_commits("patch-copy");
    let (second, third) = (commit(&st, "Add the second line"), commit(&st, "Third: more (lines)"));
    let fx = run(&mut st, PatchMsg::Copy(vec![third, second]));
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::Copy { oids, .. })] if *oids == [second, third]), "{fx:?}");

    let kept = finish_keeping(&mut st, fx);
    let copied: Vec<&String> = kept.iter().filter_map(|e| if let Effect::Ui(UiEffect::CopyText(t)) = e { Some(t) } else { None }).collect();
    assert_eq!(copied.len(), 1);
    assert!(copied[0].contains("Subject: [PATCH] Add the second line") && copied[0].contains("Subject: [PATCH] Third"));
    assert_eq!(last_toast(&st), (true, "Copied the patches of 2 commits to the clipboard".to_owned()));

    let text = format!("From {third} Mon Sep 17 00:00:00 2001\n");
    let fx = run(&mut st, PatchMsg::Copied(Ok((text.clone(), ExportReport { written: 1, skipped: Vec::new() }))));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::CopyText(t))] if *t == text));
    assert_eq!(last_toast(&st).1, format!("Copied the patch of {} to the clipboard", &third.to_string()[..7]));

    let refused = "The patch is larger than 4 MB; save it to a file instead.";
    assert!(run(&mut st, PatchMsg::Copied(Err(refused.into()))).is_empty(), "no clipboard change");
    assert_eq!(last_toast(&st), (false, refused.to_owned()));
    cleanup(&dir);
}

#[test]
fn copy_refuses_merges() {
    let (mut st, dir) = with_merge("patch-copy-merge");
    let merge = commit(&st, "Merge side");
    assert!(run(&mut st, PatchMsg::Copy(vec![merge])).is_empty());
    assert_eq!(last_log(&st), PATCH_BLOCKED_MERGE);
    assert!(run(&mut st, PatchMsg::Copy(vec![Oid::zero()])).is_empty());
    cleanup(&dir);
}

fn labels(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).collect()
}

#[test]
fn palette_patch_entries() {
    let (mut st, dir) = with_merge("patch-palette");
    let wip = "Create patch from uncommitted changes…";
    assert!(!labels(&st).iter().any(|l| l == wip), "clean");
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(labels(&st).iter().any(|l| l == wip));

    let (second, third, merge) = (commit(&st, "Add the second line"), commit(&st, "Third: more (lines)"), commit(&st, "Merge side"));
    st.selection = MultiSelection::build([second, third], third, st.snap.as_ref().unwrap());
    assert!(labels(&st).iter().any(|l| l == "Create patch from 2 selected commits…"));
    st.selection = MultiSelection::build([second, merge], merge, st.snap.as_ref().unwrap());
    assert!(!labels(&st).iter().any(|l| l.contains("selected commits")), "not with a merge");

    st.selection = Selection::Commit(third);
    let short = &third.to_string()[..7];
    let all = labels(&st);
    assert!(
        all.contains(&format!("Create patch from commit {short}…")) && all.contains(&format!("Copy patch of commit {short}")),
        "{all:?}"
    );
    st.selection = Selection::Commit(merge);
    assert!(!labels(&st).iter().any(|l| l.starts_with("Copy patch of commit")));

    // Choosing one sends its message.
    st.selection = Selection::Commit(third);
    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Patch(PatchMsg::Copy(vec![third])))), Instant::now());
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::Copy { oids, .. })] if *oids == [third]), "{fx:?}");
    let fx = update(
        &mut st,
        Msg::Palette(PaletteMsg::RunPaletteCommand(C::Patch(PatchMsg::ChangesDialog { scope: WipScope::All, path: None }))),
        Instant::now(),
    );
    assert!(fx.is_empty() && matches!(st.modal, Some(crate::state::Modal::SavePatch(_))));
    cleanup(&dir);
}
