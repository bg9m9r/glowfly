//! The TAGS row menu: annotate (lightweight tags only), hide or solo in the graph, push to each
//! remote or all of them, delete locally, delete from each remote or all of them, which items
//! show for one remote or none, the remote items disabled while a job runs, the remote delete
//! confirmation, and the markers on hidden and soloed tag rows. Remotes are injected into the
//! shared fixture's snapshot so the images are deterministic.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_remotes::{click_steps, running};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{GraphMsg, Msg};
use crate::state::tags::delete_remote_confirm;
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::LoadOptions;
use trellis_core::settings::{FilterOp, GraphFilter, ThemeChoice};

const ITEMS: [&str; 12] = [
    "Checkout (detached)",
    "Annotate…",
    "Hide in graph",
    "Solo",
    "Copy link to this tag",
    "Push to origin",
    "Push to backup",
    "Push to all remotes",
    "Delete locally",
    "Delete from origin",
    "Delete from backup",
    "Delete from all remotes",
];

/// The fixture with the TAGS section open and the given remotes (no branches on them).
fn with_tags(theme: ThemeChoice, name: &str, remotes: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.snap.as_mut().unwrap().remotes = remotes.iter().map(|r| ((*r).to_owned(), Vec::new())).collect();
    st.view.sections.tags = true;
    (st, dir)
}

/// The middle of the sidebar row for tag `name`: the leftmost match, since the graph's ref chip
/// may carry the same label.
fn tag_row(h: &H, name: &str) -> Pos2 {
    let r = h.get_all_by_label(name).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the row");
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

fn open_menu(h: &mut H) {
    let at = tag_row(h, "v0.1.0");
    click(h, at, PointerButton::Secondary);
}

fn both() -> Vec<String> {
    vec!["origin".into(), "backup".into()]
}

fn filter_pushed(h: &H, op: FilterOp) -> bool {
    pushed(h, &|m| matches!(m, Msg::Graph(GraphMsg::GraphFilter(o)) if *o == op))
}

#[test]
fn tag_row_menu() {
    let (st, dir) = with_tags(ThemeChoice::Dark, "tag-row-menu", &["origin", "backup"]);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    for item in ITEMS {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    h.check_snapshot("tag_menu");
    h.get_by_label("Delete from all remotes").click();
    h.run();
    let want = TagMsg::DeleteRemote { tag: "v0.1.0".into(), remotes: both() };
    assert!(pushed(&h, &|m| matches!(m, Msg::Confirm { then, .. } if matches!(&**then, Msg::Tag(t) if *t == want))));

    open_menu(&mut h);
    h.get_by_label("Push to all remotes").click();
    h.run();
    let want = TagMsg::Push { tag: "v0.1.0".into(), remotes: both() };
    assert!(pushed(&h, &|m| matches!(m, Msg::Tag(t) if *t == want)));

    open_menu(&mut h);
    h.get_by_label("Push to backup").click();
    h.run();
    let want = TagMsg::Push { tag: "v0.1.0".into(), remotes: vec!["backup".into()] };
    assert!(pushed(&h, &|m| matches!(m, Msg::Tag(t) if *t == want)));

    open_menu(&mut h);
    h.get_by_label("Checkout (detached)").click();
    h.run();
    let want = TagMsg::Checkout { tag: "v0.1.0".into() };
    assert!(pushed(&h, &|m| matches!(m, Msg::Tag(t) if *t == want)));

    open_menu(&mut h);
    h.get_by_label("Annotate…").click();
    h.run();
    let want = TagMsg::OpenAnnotate { tag: "v0.1.0".into() };
    assert!(pushed(&h, &|m| matches!(m, Msg::Tag(t) if *t == want)));

    open_menu(&mut h);
    h.get_by_label("Copy link to this tag").click();
    h.run();
    let want = crate::deep_link::LinkTarget::Tag("v0.1.0".into());
    assert!(pushed(&h, &|m| matches!(m, Msg::DeepLink(crate::deep_link::DeepLinkMsg::Copy(t)) if *t == want)));

    open_menu(&mut h);
    h.get_by_label("Hide in graph").click();
    h.run();
    assert!(filter_pushed(&h, FilterOp::Hide("refs/tags/v0.1.0".into())));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_row_menu_one_remote() {
    let (st, dir) = with_tags(ThemeChoice::Dark, "tag-row-menu-one", &["origin"]);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    for item in ["Checkout (detached)", "Push to origin", "Delete locally", "Delete from origin"] {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    for item in ["Push to all remotes", "Delete from all remotes"] {
        assert!(h.query_by_label(item).is_none(), "{item}");
    }
    assert!(!h.get_by_label("Delete from origin").is_disabled());
    h.press_key(Key::Escape);
    h.run();

    // While a job runs, the remote items are disabled; the spinner keeps repainting.
    running(&mut h.state_mut().0);
    h.run_steps(2);
    let at = tag_row(&h, "v0.1.0");
    click_steps(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("Delete from origin").is_disabled(), "a job is running");
    assert!(h.get_by_label("Push to origin").is_disabled());
    assert!(!h.get_by_label("Delete locally").is_disabled());
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = with_tags(ThemeChoice::Dark, "tag-row-menu-none", &[]);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.query_by_label("Checkout (detached)").is_some());
    assert!(h.query_by_label("Delete locally").is_some());
    for item in ["Push to all remotes", "Delete from all remotes"] {
        assert!(h.query_by_label(item).is_none(), "{item}");
    }
    assert!(h.query_by_label_contains("Push to").is_none() && h.query_by_label_contains("Delete from").is_none());
    h.get_by_label("Delete locally").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Confirm { title, button, .. } if title == "Delete tag" && button == "Delete")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_delete_remote_confirm_light() {
    let (mut st, dir) = with_tags(ThemeChoice::Light, "tag-delete-remote-confirm", &["origin", "backup"]);
    let Msg::Confirm { title, body, button, then } = delete_remote_confirm("v0.1.0", both()) else { panic!("a confirmation") };
    st.modal = Some(Modal::Confirm { title, body, button, then });
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label_contains("Your local tag stays").is_some());
    h.check_snapshot("tag_delete_remote_confirm_light");
    h.get_by_label("Delete from all remotes").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_row_menu_annotated_tag() {
    // The fixture's v0.1.0 is lightweight; as an annotated tag it offers no Annotate item.
    let (mut st, dir) = with_tags(ThemeChoice::Dark, "tag-row-menu-annotated", &["origin"]);
    st.snap.as_mut().unwrap().lightweight_tags.remove("v0.1.0");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.query_by_label("Checkout (detached)").is_some());
    assert!(h.query_by_label("Annotate…").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tags_hidden_solo() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tags-hidden-solo");
    git(&dir, &["tag", "-a", "v0.2.0", "-m", "Second", "search"], "2026-01-05T10:00:00+00:00");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let filter = GraphFilter { hidden: vec!["refs/tags/v0.1.0".into()], solo: vec!["refs/tags/v0.2.0".into()] };
    st.settings.set_graph_filter(&workdir, filter.clone());
    let opts = LoadOptions { max_commits: 500, hidden: filter.hidden, solo: filter.solo, ..Default::default() };
    reload(&mut st, &dir, &opts);
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    st.view.sections.tags = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tags_hidden_solo");
    assert!(h.query_by_label("Soloing v0.2.0").is_some());
    assert!(h.query_by_label("1 hidden from the graph").is_some());
    h.get_by_label("Show v0.1.0 in graph").click();
    h.run();
    assert!(filter_pushed(&h, FilterOp::Show("refs/tags/v0.1.0".into())));

    let at = tag_row(&h, "v0.2.0");
    click(&mut h, at, PointerButton::Secondary);
    assert_eq!(h.get_all_by_label("Stop soloing v0.2.0").count(), 1, "the row's marker");
    assert_eq!(h.get_all_by_label("Stop soloing").count(), 2, "the banner's button and the menu item");
    assert!(!h.get_by_label("Hide in graph").is_disabled());
    drop(h);
    remove_fixture(&dir);
}
