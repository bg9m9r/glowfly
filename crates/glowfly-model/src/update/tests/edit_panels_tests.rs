//! Headless tests for the panels' edits (update/edit.rs): the palette's query (dropped for a
//! palette step since left, the selection back at the top only when it changed, a sub-list
//! ranked for it after the update), the pull request's forms and line comment, the issue's
//! comment box, the rebase editor's draft and the conflict output's buffer, each dropped once
//! the thing it was drawn from has gone.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{EditMsg, Msg};
use crate::state::AppState;
use crate::state::ForgeState;
use crate::state::conflict::{ConflictView, OutputEdit};
use crate::state::issue::IssueView;
use crate::state::issue_comment::CommentFields;
use crate::state::op_fixture::snapshot;
use crate::state::palette::{Items, Mode, Palette, Pick, PickKind};
use crate::state::pull_request::PrView;
use crate::state::rebase::{Editing, RebasePanel};
use crate::state::session::Gen;
use glowfly_core::forge::{Issue, Provider, PullRequest, Target};
use glowfly_core::rebase::RebaseBase;
use std::time::Instant;

fn edit(st: &mut AppState, e: EditMsg) -> Vec<Effect> {
    update(st, Msg::Edit(e), Instant::now())
}

fn query(st: &AppState) -> (&str, usize) {
    let p = st.palette.as_ref().expect("open");
    (p.query.as_str(), p.selected)
}

fn target() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn forge() -> ForgeState {
    ForgeState::new(Some(target()), "origin".into(), Some(target()))
}

fn pull_request(id: u64) -> PullRequest {
    PullRequest {
        id,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

#[test]
fn the_palette_query_lands_on_its_own_step() {
    let mut st = state();
    st.palette = Some(Palette { selected: 3, ..Default::default() });
    let generation = st.palette_gen;
    edit(&mut st, EditMsg::PaletteQuery { generation, query: String::new() });
    assert_eq!(query(&st), ("", 3), "the same query keeps the selection");
    edit(&mut st, EditMsg::PaletteQuery { generation, query: "pu".into() });
    assert_eq!(query(&st), ("pu", 0), "a new query starts at the top");
    edit(&mut st, EditMsg::PaletteQuery { generation: generation + 1, query: "stale".into() });
    assert_eq!(query(&st), ("pu", 0), "typed into a step since left");
}

#[test]
fn a_sub_list_is_ranked_for_the_query_after_the_update() {
    let mut st = state();
    let mut pk = Pick::new(PickKind::FileHistory);
    pk.set_items(Items::Ready(vec!["src/app.rs".into(), "docs/guide.md".into(), "src/main.rs".into()]));
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let generation = st.palette_gen;
    edit(&mut st, EditMsg::PaletteQuery { generation, query: "src".into() });
    let Some(Palette { mode: Mode::Pick(pk), .. }) = &st.palette else { panic!("the sub-list") };
    let ranked = pk.ranked().expect("ranked by the update");
    assert_eq!((ranked.query.as_str(), ranked.total), ("src", 2));
}

#[test]
fn the_pull_request_forms_land_on_the_same_pull_request() {
    let mut st = state();
    let (fs, snap) = (forge(), snapshot("edit-pr"));
    st.session.pull_request.view = Some(PrView::new(pull_request(7), &fs, &snap));
    let v = st.session.pull_request.view.as_ref().unwrap();
    let mut f = v.fields();
    f.forms.comment = "Looks good".into();
    f.forms.replies.insert("t1".into(), "Fixed".into());
    f.line_text = Some("not open".into());
    edit(&mut st, EditMsg::PullRequest(Box::new(f.clone())));
    let v = st.session.pull_request.view.as_ref().unwrap();
    assert_eq!((v.forms.comment.as_str(), v.forms.replies.get("t1").map(String::as_str)), ("Looks good", Some("Fixed")));
    assert!(v.line_draft.is_none(), "no line comment box was open");

    st.session.pull_request.view = Some(PrView::new(pull_request(8), &fs, &snap));
    edit(&mut st, EditMsg::PullRequest(Box::new(f)));
    assert_eq!(st.session.pull_request.view.as_ref().unwrap().forms.comment, "", "another pull request is open");
}

#[test]
fn the_issue_comment_lands_on_the_same_issue() {
    let mut st = state();
    let fs = forge();
    let issue = |key: &str| Issue {
        key: key.into(),
        title: "Crash".into(),
        state: "open".into(),
        kind: String::new(),
        assignee: None,
        url: String::new(),
        facets: Default::default(),
    };
    st.session.issue.view = IssueView::new(issue("12"), &fs);
    let fields = CommentFields { text: "Seen it too".into(), touched: Default::default() };
    edit(&mut st, EditMsg::IssueComment { key: "13".into(), fields: fields.clone() });
    assert_eq!(st.session.issue.view.as_ref().unwrap().comment.text, "", "another issue's box");
    edit(&mut st, EditMsg::IssueComment { key: "12".into(), fields });
    assert_eq!(st.session.issue.view.as_ref().unwrap().comment.text, "Seen it too");
}

#[test]
fn the_rebase_draft_lands_on_the_open_editor_only() {
    let mut st = state();
    let mut panel = RebasePanel::loading(RebaseBase::OntoBranch("main".into()));
    panel.editing = Some(Editing::Message(1));
    st.session.rebase = Gen::open(panel);
    edit(&mut st, EditMsg::RebaseDraft { editing: Editing::Message(2), draft: "other row".into() });
    assert_eq!(st.session.rebase.view.as_ref().unwrap().draft, "");
    edit(&mut st, EditMsg::RebaseDraft { editing: Editing::Message(1), draft: "Reword it".into() });
    assert_eq!(st.session.rebase.view.as_ref().unwrap().draft, "Reword it");
    st.session.rebase.view.as_mut().unwrap().editing = None;
    edit(&mut st, EditMsg::RebaseDraft { editing: Editing::Message(1), draft: "late".into() });
    assert_eq!(st.session.rebase.view.as_ref().unwrap().draft, "Reword it", "the editor closed");
}

#[test]
fn the_conflict_output_lands_on_the_same_file_while_it_is_edited() {
    let mut st = state();
    st.session.conflict = Gen::open(ConflictView::loading("a.txt".into()));
    let typed = |path: &str| EditMsg::ConflictOutput { path: path.into(), buffer: "mine\n".into() };
    edit(&mut st, typed("a.txt"));
    assert!(st.session.conflict.view.as_ref().unwrap().edit.is_none(), "not being edited");

    st.session.conflict.view.as_mut().unwrap().edit = Some(OutputEdit { buffer: "base\n".into(), rendered: Default::default() });
    edit(&mut st, typed("b.txt"));
    let buffer = |st: &AppState| st.session.conflict.view.as_ref().unwrap().edit.as_ref().unwrap().buffer.clone();
    assert_eq!(buffer(&st), "base\n", "another file");
    edit(&mut st, typed("a.txt"));
    assert_eq!(buffer(&st), "mine\n");
}
